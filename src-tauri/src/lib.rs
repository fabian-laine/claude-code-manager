mod db;
mod history;
mod session;

use db::{Db, Project};
use serde_json::Value;
use session::{ClaudeEvent, ProcessRegistry};
use tauri::Emitter;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

struct AppState {
    db: Arc<Db>,
    registry: Arc<ProcessRegistry>,
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    state.db.list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<Project, String> {
    state.db.add_project(&name, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_project(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_history(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<Value>, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let sid = match project.last_session_id {
        Some(s) => s,
        None => match history::latest_session_id(&project.path) {
            Some(s) => s,
            None => return Ok(vec![]),
        },
    };
    history::load_session(&project.path, &sid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_mcp_servers(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<McpServer>, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;

    let output = tokio::process::Command::new("claude")
        .arg("mcp")
        .arg("list")
        .current_dir(&project.path)
        .output()
        .await
        .map_err(|e| format!("failed to run claude mcp list: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(parse_mcp_list(&stdout))
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpServer {
    name: String,
    status: String,
    ok: bool,
}

fn parse_mcp_list(stdout: &str) -> Vec<McpServer> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Typical format: "name: status ..." or "name: ✓ Connected"
        if let Some((name, rest)) = trimmed.split_once(':') {
            let name = name.trim().to_string();
            if name.is_empty() || name.eq_ignore_ascii_case("mcp servers") {
                continue;
            }
            let status = rest.trim().to_string();
            let ok = status.contains('✓')
                || status.to_lowercase().contains("connected")
                || status.to_lowercase().contains("ok");
            out.push(McpServer { name, status, ok });
        }
    }
    out
}

#[tauri::command]
async fn open_folder(path: String) -> Result<(), String> {
    tokio::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    prompt: String,
) -> Result<(), String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let resume = project.last_session_id.clone();
    let db = state.db.clone();
    let registry = state.registry.clone();
    let pid = project_id.clone();
    tokio::spawn(async move {
        match session::run_claude(app, registry, pid.clone(), project.path, prompt, resume).await {
            Ok(Some(sid)) => {
                let _ = db.set_last_session(&pid, &sid);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("claude run error: {}", e);
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn cancel_message(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    state
        .registry
        .cancel(&project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn pause_message(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    let ok = state
        .registry
        .pause(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    if ok {
        let _ = app.emit(
            "claude-event",
            ClaudeEvent::Paused {
                project_id: project_id.clone(),
            },
        );
    }
    Ok(ok)
}

#[tauri::command]
async fn resume_message(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    let ok = state
        .registry
        .resume(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    if ok {
        let _ = app.emit(
            "claude-event",
            ClaudeEvent::Resumed {
                project_id: project_id.clone(),
            },
        );
    }
    Ok(ok)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let mut db_path = app
                .path()
                .app_data_dir()
                .expect("no app data dir");
            db_path.push("ccm.sqlite");
            let db = Db::open(db_path).expect("failed to open db");
            app.manage(AppState {
                db: Arc::new(db),
                registry: Arc::new(ProcessRegistry::default()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            add_project,
            delete_project,
            load_history,
            send_message,
            cancel_message,
            pause_message,
            resume_message,
            list_mcp_servers,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
