mod db;
mod history;
mod server;
mod session;
mod stt;

use db::{Db, Project};
use server::{ServerHandle, ServerState, TlsPaths};
use session::{new_event_bus, ClaudeEvent, EventBus, ProcessRegistry};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

struct AppState {
    db: Arc<Db>,
    registry: Arc<ProcessRegistry>,
    bus: EventBus,
    server: Arc<Mutex<Option<ServerHandle>>>,
    token: Arc<Mutex<String>>,
    frontend_dir: PathBuf,
    stt_model_path: PathBuf,
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
fn clear_session(state: State<'_, AppState>, project_id: String) -> Result<(), String> {
    state
        .db
        .clear_last_session(&project_id)
        .map_err(|e| e.to_string())
}

fn sanitize_attachment_name(name: &str) -> String {
    // Keep only the basename, and replace anything awkward with '_'.
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn save_attachment_to_project(
    project_path: &str,
    filename: &str,
    data: &[u8],
) -> Result<String, String> {
    let dir = PathBuf::from(project_path).join(".ccm-attachments");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe = sanitize_attachment_name(filename);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let final_name = format!("{}-{}", stamp, safe);
    let path = dir.join(&final_name);
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(format!(".ccm-attachments/{}", final_name))
}

#[tauri::command]
fn save_attachment(
    state: State<'_, AppState>,
    project_id: String,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    save_attachment_to_project(&project.path, &filename, &data)
}

#[tauri::command]
fn copy_attachment(
    state: State<'_, AppState>,
    project_id: String,
    src_path: String,
) -> Result<String, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let src = std::path::Path::new(&src_path);
    let filename = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let data = std::fs::read(&src).map_err(|e| format!("read {}: {}", src.display(), e))?;
    save_attachment_to_project(&project.path, &filename, &data)
}

#[derive(serde::Serialize)]
struct SttStatus {
    available: bool,
    model_ready: bool,
    model_path: String,
}

#[tauri::command]
fn stt_status(state: State<'_, AppState>) -> Result<SttStatus, String> {
    let path = state.stt_model_path.clone();
    Ok(SttStatus {
        available: stt::is_available(),
        model_ready: path.exists(),
        model_path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
async fn stt_download_model(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.stt_model_path.clone();
    if path.exists() {
        return Ok(());
    }
    stt::download_model(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn stt_transcribe(
    state: State<'_, AppState>,
    wav: Vec<u8>,
    lang: Option<String>,
) -> Result<String, String> {
    let path = state.stt_model_path.clone();
    tokio::task::spawn_blocking(move || {
        stt::transcribe_wav(&wav, &path, lang.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn load_history_chunk(
    state: State<'_, AppState>,
    project_id: String,
    before_ts: Option<String>,
    hours: Option<u32>,
) -> Result<history::HistoryChunk, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let sid = match project.last_session_id {
        Some(s) => s,
        None => match history::latest_session_id(&project.path) {
            Some(s) => s,
            None => {
                return Ok(history::HistoryChunk {
                    events: vec![],
                    oldest_ts: None,
                    has_more: false,
                    session_id: None,
                })
            }
        },
    };
    history::load_session_chunk(
        &project.path,
        &sid,
        before_ts.as_deref(),
        hours.unwrap_or(2),
    )
    .map_err(|e| e.to_string())
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

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[tauri::command]
fn fs_read(path: String) -> Result<String, String> {
    let p = expand_home(&path);
    if !p.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&p).map_err(|e| format!("read {}: {}", p.display(), e))
}

#[tauri::command]
fn fs_write(path: String, content: String) -> Result<(), String> {
    let p = expand_home(&path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }
    std::fs::write(&p, content).map_err(|e| format!("write {}: {}", p.display(), e))
}

#[derive(serde::Serialize)]
struct FsEntry {
    name: String,
    path: String,
    is_dir: bool,
}

#[tauri::command]
fn fs_list(path: String) -> Result<Vec<FsEntry>, String> {
    let p = expand_home(&path);
    if !p.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let rd = std::fs::read_dir(&p).map_err(|e| format!("read_dir {}: {}", p.display(), e))?;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = path.is_dir();
        out.push(FsEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[derive(serde::Serialize, Default, Clone)]
struct SessionStats {
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    last_context_tokens: u64,
    model: Option<String>,
    user_messages: u32,
    assistant_messages: u32,
    session_id: Option<String>,
}

fn parse_session_stats(path: &std::path::Path) -> SessionStats {
    let mut stats = SessionStats::default();
    let Ok(content) = std::fs::read_to_string(path) else {
        return stats;
    };
    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if t == "assistant" {
            stats.assistant_messages += 1;
            if let Some(msg) = v.get("message") {
                if let Some(model) = msg.get("model").and_then(|m| m.as_str()) {
                    stats.model = Some(model.to_string());
                }
                if let Some(usage) = msg.get("usage") {
                    let inp = usage
                        .get("input_tokens")
                        .and_then(|u| u.as_u64())
                        .unwrap_or(0);
                    let out = usage
                        .get("output_tokens")
                        .and_then(|u| u.as_u64())
                        .unwrap_or(0);
                    let cc = usage
                        .get("cache_creation_input_tokens")
                        .and_then(|u| u.as_u64())
                        .unwrap_or(0);
                    let cr = usage
                        .get("cache_read_input_tokens")
                        .and_then(|u| u.as_u64())
                        .unwrap_or(0);
                    stats.input_tokens += inp;
                    stats.output_tokens += out;
                    stats.cache_creation_tokens += cc;
                    stats.cache_read_tokens += cr;
                    // Context occupancy ≈ last turn's total input (inp + cache reads + cache writes).
                    stats.last_context_tokens = inp + cc + cr;
                }
            }
        } else if t == "user" {
            if let Some(msg) = v.get("message") {
                if let Some(content) = msg.get("content") {
                    // Skip tool_result-only user messages; only count real user turns.
                    let is_tool_result_only = content
                        .as_array()
                        .map(|arr| {
                            arr.iter().all(|b| {
                                b.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                            })
                        })
                        .unwrap_or(false);
                    if !is_tool_result_only {
                        stats.user_messages += 1;
                    }
                }
            }
        }
    }
    stats
}

#[tauri::command]
fn session_stats(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<SessionStats, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let sid = match project.last_session_id {
        Some(s) => s,
        None => match history::latest_session_id(&project.path) {
            Some(s) => s,
            None => return Ok(SessionStats::default()),
        },
    };
    let Some(dir) = history::encoded_project_dir(&project.path) else {
        return Ok(SessionStats::default());
    };
    let file = dir.join(format!("{}.jsonl", sid));
    let mut stats = parse_session_stats(&file);
    stats.session_id = Some(sid);
    Ok(stats)
}

#[derive(serde::Serialize, Default)]
struct GlobalStats {
    total_sessions: u32,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cache_creation_tokens: u64,
    total_cache_read_tokens: u64,
    by_project: Vec<ProjectStats>,
}

#[derive(serde::Serialize)]
struct ProjectStats {
    name: String,
    sessions: u32,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
}

#[tauri::command]
fn global_stats(state: State<'_, AppState>) -> Result<GlobalStats, String> {
    let projects = state.db.list_projects().map_err(|e| e.to_string())?;
    let mut out = GlobalStats::default();
    for p in &projects {
        let Some(dir) = history::encoded_project_dir(&p.path) else { continue };
        if !dir.exists() {
            continue;
        }
        let mut pstats = ProjectStats {
            name: p.name.clone(),
            sessions: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        };
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                pstats.sessions += 1;
                let s = parse_session_stats(&path);
                pstats.input_tokens += s.input_tokens;
                pstats.output_tokens += s.output_tokens;
                pstats.cache_creation_tokens += s.cache_creation_tokens;
                pstats.cache_read_tokens += s.cache_read_tokens;
            }
        }
        out.total_sessions += pstats.sessions;
        out.total_input_tokens += pstats.input_tokens;
        out.total_output_tokens += pstats.output_tokens;
        out.total_cache_creation_tokens += pstats.cache_creation_tokens;
        out.total_cache_read_tokens += pstats.cache_read_tokens;
        if pstats.sessions > 0 {
            out.by_project.push(pstats);
        }
    }
    out.by_project
        .sort_by(|a, b| (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens)));
    Ok(out)
}

#[derive(serde::Serialize, Default)]
struct ProjectFlags {
    model: Option<String>,
    effort: Option<String>,
    add_dirs: Vec<String>,
}

fn project_flag_key(project_id: &str, name: &str) -> String {
    format!("project.{}.{}", project_id, name)
}

fn load_project_flags(db: &Db, project_id: &str) -> ProjectFlags {
    let model = db
        .get_setting(&project_flag_key(project_id, "model"))
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let effort = db
        .get_setting(&project_flag_key(project_id, "effort"))
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let add_dirs = db
        .get_setting(&project_flag_key(project_id, "add_dirs"))
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();
    ProjectFlags {
        model,
        effort,
        add_dirs,
    }
}

#[tauri::command]
fn get_project_flags(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectFlags, String> {
    Ok(load_project_flags(&state.db, &project_id))
}

#[tauri::command]
fn set_project_flag(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    value: String,
) -> Result<(), String> {
    if !["model", "effort"].contains(&name.as_str()) {
        return Err(format!("unknown flag: {}", name));
    }
    state
        .db
        .set_setting(&project_flag_key(&project_id, &name), &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project_dir(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<Vec<String>, String> {
    let mut flags = load_project_flags(&state.db, &project_id);
    if !flags.add_dirs.iter().any(|p| p == &path) {
        flags.add_dirs.push(path);
    }
    let serialized = serde_json::to_string(&flags.add_dirs).map_err(|e| e.to_string())?;
    state
        .db
        .set_setting(&project_flag_key(&project_id, "add_dirs"), &serialized)
        .map_err(|e| e.to_string())?;
    Ok(flags.add_dirs)
}

#[tauri::command]
fn remove_project_dir(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<Vec<String>, String> {
    let mut flags = load_project_flags(&state.db, &project_id);
    flags.add_dirs.retain(|p| p != &path);
    let serialized = serde_json::to_string(&flags.add_dirs).map_err(|e| e.to_string())?;
    state
        .db
        .set_setting(&project_flag_key(&project_id, "add_dirs"), &serialized)
        .map_err(|e| e.to_string())?;
    Ok(flags.add_dirs)
}

#[tauri::command]
async fn git_diff(state: State<'_, AppState>, project_id: String) -> Result<String, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let output = tokio::process::Command::new("git")
        .arg("diff")
        .arg("HEAD")
        .current_dir(&project.path)
        .output()
        .await
        .map_err(|e| format!("git diff failed: {}", e))?;
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff failed: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
async fn send_message(
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
    let flags = load_project_flags(&state.db, &project_id);
    let db = state.db.clone();
    let bus = state.bus.clone();
    let registry = state.registry.clone();
    let pid = project_id.clone();
    tokio::spawn(async move {
        match session::run_claude(
            bus,
            registry,
            pid.clone(),
            project.path,
            prompt,
            resume,
            session::RunFlags {
                model: flags.model,
                effort: flags.effort,
                add_dirs: flags.add_dirs,
            },
        )
        .await
        {
            Ok(Some(sid)) => {
                let _ = db.set_last_session(&pid, &sid);
            }
            Ok(None) => {}
            Err(e) => eprintln!("claude run error: {}", e),
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
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    let ok = state
        .registry
        .pause(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    if ok {
        let _ = state.bus.send(ClaudeEvent::Paused {
            project_id: project_id.clone(),
        });
    }
    Ok(ok)
}

#[tauri::command]
async fn resume_message(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    let ok = state
        .registry
        .resume(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    if ok {
        let _ = state.bus.send(ClaudeEvent::Resumed {
            project_id: project_id.clone(),
        });
    }
    Ok(ok)
}

#[derive(serde::Serialize, Clone)]
struct RemoteStatus {
    enabled: bool,
    port: u16,
    token: String,
    https: bool,
    cert_ready: bool,
    cert_hostname: Option<String>,
}

fn tls_cert_dir(db: &Db) -> Option<PathBuf> {
    db.get_setting("remote.cert_dir").ok().flatten().map(PathBuf::from)
}

fn tls_paths(db: &Db) -> Option<TlsPaths> {
    let hostname = db.get_setting("remote.cert_hostname").ok().flatten()?;
    let dir = tls_cert_dir(db)?;
    let cert = dir.join(format!("{}.crt", hostname));
    let key = dir.join(format!("{}.key", hostname));
    if cert.exists() && key.exists() {
        Some(TlsPaths { cert, key })
    } else {
        None
    }
}

#[tauri::command]
async fn remote_status(state: State<'_, AppState>) -> Result<RemoteStatus, String> {
    let server = state.server.lock().await;
    let token = state.token.lock().await.clone();
    let enabled = server.is_some();
    let port = server.as_ref().map(|h| h.port).unwrap_or_else(|| {
        state
            .db
            .get_setting("remote.port")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(17890)
    });
    let https = server.as_ref().map(|h| h.https).unwrap_or(false);
    let cert_hostname = state.db.get_setting("remote.cert_hostname").ok().flatten();
    let cert_ready = tls_paths(&state.db).is_some();
    Ok(RemoteStatus {
        enabled,
        port,
        token,
        https,
        cert_ready,
        cert_hostname,
    })
}

#[tauri::command]
async fn remote_start(
    state: State<'_, AppState>,
    port: Option<u16>,
    https: Option<bool>,
) -> Result<RemoteStatus, String> {
    let mut guard = state.server.lock().await;
    if guard.is_some() {
        return Err("server already running".to_string());
    }
    let port = port.unwrap_or(17890);

    {
        let mut tok = state.token.lock().await;
        if tok.is_empty() {
            let new_token = server::generate_token();
            state
                .db
                .set_setting("remote.token", &new_token)
                .map_err(|e| e.to_string())?;
            *tok = new_token;
        }
    }

    let tls = if https.unwrap_or(false) {
        match tls_paths(&state.db) {
            Some(p) => Some(p),
            None => {
                return Err(
                    "HTTPS requested but no TLS cert found — generate one first."
                        .to_string(),
                )
            }
        }
    } else {
        None
    };

    let server_state = ServerState {
        db: state.db.clone(),
        registry: state.registry.clone(),
        bus: state.bus.clone(),
        token: state.token.clone(),
        frontend_dir: state.frontend_dir.clone(),
        stt_model_path: state.stt_model_path.clone(),
    };

    let handle = server::start(server_state, port, tls)
        .await
        .map_err(|e| e.to_string())?;
    let bound_port = handle.port;
    let is_https = handle.https;

    state
        .db
        .set_setting("remote.enabled", "true")
        .map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("remote.port", &bound_port.to_string())
        .map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("remote.https", if is_https { "true" } else { "false" })
        .map_err(|e| e.to_string())?;

    *guard = Some(handle);

    let cert_hostname = state.db.get_setting("remote.cert_hostname").ok().flatten();
    let cert_ready = tls_paths(&state.db).is_some();

    Ok(RemoteStatus {
        enabled: true,
        port: bound_port,
        token: state.token.lock().await.clone(),
        https: is_https,
        cert_ready,
        cert_hostname,
    })
}

#[tauri::command]
async fn remote_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.server.lock().await;
    if let Some(handle) = guard.take() {
        handle.handle.shutdown();
    }
    state
        .db
        .set_setting("remote.enabled", "false")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
struct TlsCertResult {
    hostname: String,
    cert_path: String,
    key_path: String,
}

#[tauri::command]
async fn remote_generate_tls_cert(
    app: AppHandle,
    state: State<'_, AppState>,
    hostname: String,
) -> Result<TlsCertResult, String> {
    if hostname.trim().is_empty() {
        return Err("hostname is empty".to_string());
    }

    // Store certs under the app data dir so they survive upgrades.
    let mut cert_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("no app data dir: {}", e))?;
    cert_dir.push("certs");
    std::fs::create_dir_all(&cert_dir).map_err(|e| e.to_string())?;

    let cert_path = cert_dir.join(format!("{}.crt", hostname));
    let key_path = cert_dir.join(format!("{}.key", hostname));

    let output = tokio::process::Command::new("tailscale")
        .arg("cert")
        .arg("--cert-file")
        .arg(&cert_path)
        .arg("--key-file")
        .arg(&key_path)
        .arg(&hostname)
        .output()
        .await
        .map_err(|e| format!("failed to run tailscale cert: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale cert failed: {}", stderr));
    }

    state
        .db
        .set_setting("remote.cert_hostname", &hostname)
        .map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("remote.cert_dir", &cert_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;

    Ok(TlsCertResult {
        hostname,
        cert_path: cert_path.to_string_lossy().to_string(),
        key_path: key_path.to_string_lossy().to_string(),
    })
}

#[derive(serde::Serialize)]
struct RemoteUrls {
    hostname: Option<String>,
    tailscale: Option<String>,
}

#[tauri::command]
async fn remote_urls() -> Result<RemoteUrls, String> {
    let hostname = tokio::process::Command::new("hostname")
        .output()
        .await
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });

    let tailscale = tokio::process::Command::new("tailscale")
        .arg("status")
        .arg("--json")
        .output()
        .await
        .ok()
        .and_then(|o| {
            if !o.status.success() {
                return None;
            }
            let json: serde_json::Value =
                serde_json::from_slice(&o.stdout).ok()?;
            let dns = json.get("Self")?.get("DNSName")?.as_str()?.to_string();
            // Remove trailing dot.
            Some(dns.trim_end_matches('.').to_string())
        });

    Ok(RemoteUrls {
        hostname,
        tailscale,
    })
}

#[tauri::command]
async fn remote_rotate_token(state: State<'_, AppState>) -> Result<String, String> {
    let new_token = server::generate_token();
    state
        .db
        .set_setting("remote.token", &new_token)
        .map_err(|e| e.to_string())?;
    *state.token.lock().await = new_token.clone();
    Ok(new_token)
}

/// Forward bus events to the Tauri webview so the desktop UI keeps working.
fn spawn_bus_forwarder(app: AppHandle, bus: EventBus) {
    let mut rx = bus.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            let _ = app.emit("claude-event", ev);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // On Linux/WebKitGTK, getUserMedia silently fails unless we
            // explicitly grant permission requests. There's no native UI
            // prompt in WebKitGTK, so we hook the signal and auto-allow
            // (this app is local — the only origin loading is our own UI).
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager as _;
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.with_webview(|webview| {
                        use webkit2gtk::{PermissionRequestExt, WebViewExt};
                        webview.inner().connect_permission_request(|_w, req| {
                            req.allow();
                            true
                        });
                    });
                }
            }

            let frontend_dir = resolve_frontend_dir(&app.handle());
            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            let mut db_path = app_data_dir.clone();
            db_path.push("ccm.sqlite");
            let db = Arc::new(Db::open(db_path).expect("failed to open db"));

            let bus = new_event_bus();

            // Load persisted token (or empty; generated on first start).
            let token = db.get_setting("remote.token").ok().flatten().unwrap_or_default();
            let token = Arc::new(Mutex::new(token));

            let stt_model_path = stt::model_path_for(&app_data_dir);

            let state = AppState {
                db: db.clone(),
                registry: Arc::new(ProcessRegistry::default()),
                bus: bus.clone(),
                server: Arc::new(Mutex::new(None)),
                token: token.clone(),
                frontend_dir: frontend_dir.clone(),
                stt_model_path: stt_model_path.clone(),
            };
            app.manage(state);

            spawn_bus_forwarder(app.handle().clone(), bus.clone());

            // Auto-start the remote server if it was enabled last time.
            if db.get_setting("remote.enabled").ok().flatten().as_deref() == Some("true") {
                let port = db
                    .get_setting("remote.port")
                    .ok()
                    .flatten()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(17890);
                let token_arc = token.clone();
                let db_c = db.clone();
                let bus_c = bus.clone();
                let handle_arc: Arc<Mutex<Option<ServerHandle>>> =
                    app.state::<AppState>().inner().server.clone();
                let registry_c = app.state::<AppState>().inner().registry.clone();
                let frontend_dir_c = frontend_dir.clone();
                let stt_path_c = stt_model_path.clone();
                let wants_https = db.get_setting("remote.https").ok().flatten().as_deref()
                    == Some("true");
                let tls = if wants_https { tls_paths(&db) } else { None };
                tauri::async_runtime::spawn(async move {
                    {
                        let mut tok = token_arc.lock().await;
                        if tok.is_empty() {
                            let new_token = server::generate_token();
                            let _ = db_c.set_setting("remote.token", &new_token);
                            *tok = new_token;
                        }
                    }
                    let s = ServerState {
                        db: db_c,
                        registry: registry_c,
                        bus: bus_c,
                        token: token_arc,
                        frontend_dir: frontend_dir_c,
                        stt_model_path: stt_path_c,
                    };
                    match server::start(s, port, tls).await {
                        Ok(h) => *handle_arc.lock().await = Some(h),
                        Err(e) => eprintln!("remote auto-start failed: {}", e),
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            add_project,
            delete_project,
            clear_session,
            save_attachment,
            copy_attachment,
            stt_status,
            stt_download_model,
            stt_transcribe,
            load_history_chunk,
            send_message,
            cancel_message,
            pause_message,
            resume_message,
            list_mcp_servers,
            open_folder,
            git_diff,
            fs_read,
            fs_write,
            fs_list,
            session_stats,
            global_stats,
            get_project_flags,
            set_project_flag,
            add_project_dir,
            remove_project_dir,
            remote_status,
            remote_start,
            remote_stop,
            remote_rotate_token,
            remote_urls,
            remote_generate_tls_cert,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn resolve_frontend_dir(app: &AppHandle) -> PathBuf {
    // 1. Packaged app: Tauri bundles `../build` as a resource named `build`.
    if let Ok(res_dir) = app.path().resource_dir() {
        let candidate = res_dir.join("build");
        if candidate.exists() {
            return candidate;
        }
    }
    // 2. Dev/repo: cwd-relative fallbacks.
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|p| p.join("build"))
            .unwrap_or_default(),
        PathBuf::from("../build"),
        PathBuf::from("build"),
    ];
    for c in candidates.iter() {
        if c.exists() {
            return c.clone();
        }
    }
    PathBuf::from("build")
}
