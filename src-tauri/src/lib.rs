mod db;
mod history;
mod server;
mod session;

use db::{Db, Project};
use serde_json::Value;
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
    let bus = state.bus.clone();
    let registry = state.registry.clone();
    let pid = project_id.clone();
    tokio::spawn(async move {
        match session::run_claude(bus, registry, pid.clone(), project.path, prompt, resume).await {
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
            let frontend_dir = resolve_frontend_dir(&app.handle());
            let mut db_path = app.path().app_data_dir().expect("no app data dir");
            db_path.push("ccm.sqlite");
            let db = Arc::new(Db::open(db_path).expect("failed to open db"));

            let bus = new_event_bus();

            // Load persisted token (or empty; generated on first start).
            let token = db.get_setting("remote.token").ok().flatten().unwrap_or_default();
            let token = Arc::new(Mutex::new(token));

            let state = AppState {
                db: db.clone(),
                registry: Arc::new(ProcessRegistry::default()),
                bus: bus.clone(),
                server: Arc::new(Mutex::new(None)),
                token: token.clone(),
                frontend_dir: frontend_dir.clone(),
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
            load_history,
            send_message,
            cancel_message,
            pause_message,
            resume_message,
            list_mcp_servers,
            open_folder,
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
