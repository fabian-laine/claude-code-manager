use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Registry of currently running `claude` processes, keyed by project id.
/// Lets us cancel an in-flight run from a separate Tauri command.
#[derive(Default)]
pub struct ProcessRegistry {
    map: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
}

impl ProcessRegistry {
    pub async fn register(&self, project_id: String, child: Arc<Mutex<Child>>) {
        self.map.lock().await.insert(project_id, child);
    }

    pub async fn unregister(&self, project_id: &str) {
        self.map.lock().await.remove(project_id);
    }

    pub async fn cancel(&self, project_id: &str) -> Result<bool> {
        // Make sure the process is running before killing (SIGKILL still works on
        // a stopped process but it's cleaner to resume it first).
        let _ = self.resume(project_id).await;
        let entry = self.map.lock().await.get(project_id).cloned();
        if let Some(child_arc) = entry {
            let mut child = child_arc.lock().await;
            child.start_kill().map_err(|e| anyhow!("kill failed: {}", e))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn pid(&self, project_id: &str) -> Option<i32> {
        let entry = self.map.lock().await.get(project_id).cloned()?;
        let child = entry.lock().await;
        child.id().map(|p| p as i32)
    }

    pub async fn pause(&self, project_id: &str) -> Result<bool> {
        let Some(pid) = self.pid(project_id).await else {
            return Ok(false);
        };
        send_signal_tree(pid, libc::SIGSTOP);
        Ok(true)
    }

    pub async fn resume(&self, project_id: &str) -> Result<bool> {
        let Some(pid) = self.pid(project_id).await else {
            return Ok(false);
        };
        send_signal_tree(pid, libc::SIGCONT);
        Ok(true)
    }
}

/// Signal the process and all of its descendants. `claude` spawns node
/// subprocesses, and SIGSTOP on just the parent won't freeze in-flight work.
fn send_signal_tree(root_pid: i32, signal: libc::c_int) {
    let mut stack = vec![root_pid];
    let mut all = Vec::new();
    while let Some(pid) = stack.pop() {
        all.push(pid);
        for child in children_of(pid) {
            stack.push(child);
        }
    }
    // Signal deepest first so parents can't spawn new children we'd miss.
    for pid in all.iter().rev() {
        unsafe { libc::kill(*pid, signal) };
    }
}

fn children_of(pid: i32) -> Vec<i32> {
    let path = format!("/proc/{}/task/{}/children", pid, pid);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| {
            s.split_whitespace()
                .filter_map(|p| p.parse::<i32>().ok())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaudeEvent {
    Started { project_id: String },
    Raw { project_id: String, event: Value },
    Finished { project_id: String, session_id: Option<String> },
    Error { project_id: String, message: String },
    Cancelled { project_id: String },
    Paused { project_id: String },
    Resumed { project_id: String },
}

pub async fn run_claude(
    app: AppHandle,
    registry: Arc<ProcessRegistry>,
    project_id: String,
    project_path: String,
    prompt: String,
    resume_session_id: Option<String>,
) -> Result<Option<String>> {
    if !Path::new(&project_path).exists() {
        return Err(anyhow!("Project path does not exist: {}", project_path));
    }

    let _ = app.emit(
        "claude-event",
        ClaudeEvent::Started {
            project_id: project_id.clone(),
        },
    );

    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(&prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--dangerously-skip-permissions")
        .current_dir(&project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    if let Some(sid) = &resume_session_id {
        cmd.arg("--resume").arg(sid);
    }

    let mut child = cmd.spawn().map_err(|e| anyhow!("failed to spawn claude: {}", e))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;

    let child_arc = Arc::new(Mutex::new(child));
    registry.register(project_id.clone(), child_arc.clone()).await;

    let app_err = app.clone();
    let pid_err = project_id.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_err.emit(
                "claude-event",
                ClaudeEvent::Error {
                    project_id: pid_err.clone(),
                    message: line,
                },
            );
        }
    });

    let mut reader = BufReader::new(stdout).lines();
    let mut final_session_id: Option<String> = None;
    let mut cancelled = false;

    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(val) => {
                        if let Some(sid) = val.get("session_id").and_then(|v| v.as_str()) {
                            final_session_id = Some(sid.to_string());
                        }
                        let _ = app.emit(
                            "claude-event",
                            ClaudeEvent::Raw {
                                project_id: project_id.clone(),
                                event: val,
                            },
                        );
                    }
                    Err(e) => {
                        let _ = app.emit(
                            "claude-event",
                            ClaudeEvent::Error {
                                project_id: project_id.clone(),
                                message: format!("parse error: {} (line: {})", e, line),
                            },
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                // stdout closed unexpectedly (likely due to kill)
                cancelled = true;
                let _ = app.emit(
                    "claude-event",
                    ClaudeEvent::Error {
                        project_id: project_id.clone(),
                        message: format!("stream error: {}", e),
                    },
                );
                break;
            }
        }
    }

    let status = {
        let mut child = child_arc.lock().await;
        child.wait().await?
    };
    registry.unregister(&project_id).await;

    if !status.success() && !cancelled {
        // If the process was killed via signal, treat as cancelled rather than error.
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if status.signal().is_some() {
                cancelled = true;
            }
        }
        if !cancelled {
            let _ = app.emit(
                "claude-event",
                ClaudeEvent::Error {
                    project_id: project_id.clone(),
                    message: format!("claude exited with status: {}", status),
                },
            );
        }
    }

    if cancelled {
        let _ = app.emit(
            "claude-event",
            ClaudeEvent::Cancelled {
                project_id: project_id.clone(),
            },
        );
    }

    let _ = app.emit(
        "claude-event",
        ClaudeEvent::Finished {
            project_id: project_id.clone(),
            session_id: final_session_id.clone(),
        },
    );

    Ok(final_session_id)
}
