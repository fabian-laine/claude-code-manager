use crate::db::{Db, Project};
use crate::history;
use crate::session::{self, ClaudeEvent, EventBus, ProcessRegistry};
use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket},
        DefaultBodyLimit, Path as AxPath, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use crate::history::HistoryChunk;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::{header, HeaderValue};

#[derive(Clone)]
pub struct ServerState {
    pub db: Arc<Db>,
    pub registry: Arc<ProcessRegistry>,
    pub bus: EventBus,
    pub token: Arc<Mutex<String>>,
    pub frontend_dir: PathBuf,
    pub stt_model_path: PathBuf,
}

pub struct ServerHandle {
    pub handle: Handle,
    pub port: u16,
    pub https: bool,
}

pub struct TlsPaths {
    pub cert: PathBuf,
    pub key: PathBuf,
}

pub async fn start(
    state: ServerState,
    port: u16,
    tls: Option<TlsPaths>,
) -> anyhow::Result<ServerHandle> {
    // Install the default crypto provider for rustls exactly once.
    static PROVIDER_INIT: std::sync::Once = std::sync::Once::new();
    PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Fresh HTML on every request so UI updates are never stuck behind a
    // stale cached index. Hashed JS/CSS under /_app/immutable are served
    // with their own long cache from a dedicated route.
    let html_service = ServeDir::new(state.frontend_dir.clone())
        .append_index_html_on_directories(true);
    let html_with_no_cache = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        .service(html_service);

    let immutable_service = ServeDir::new(state.frontend_dir.join("_app/immutable"));
    let immutable_with_long_cache = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(immutable_service);

    let api = Router::new()
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/:id", delete(delete_project))
        .route("/projects/:id/clear", post(clear_session))
        .route(
            "/projects/:id/attachments",
            post(upload_attachment).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route("/stt/status", get(stt_status))
        .route("/stt/download", post(stt_download))
        .route("/usage", get(claude_usage_route))
        .route(
            "/stt/transcribe",
            post(stt_transcribe).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/projects/:id/history", get(load_history))
        .route("/projects/:id/messages", post(send_message))
        .route("/projects/:id/pause", post(pause_message))
        .route("/projects/:id/resume", post(resume_message))
        .route("/projects/:id/cancel", post(cancel_message))
        .route("/projects/:id/mcp", get(list_mcp_servers))
        .route("/projects/:id/diff", get(git_diff))
        .route("/events", get(ws_handler))
        .route("/ping", get(ping));

    let app = Router::new()
        .nest("/api", api)
        .nest_service("/_app/immutable", immutable_with_long_cache)
        .fallback_service(html_with_no_cache)
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let handle = Handle::new();
    let handle_spawn = handle.clone();
    let app_make = app.into_make_service();

    let https = tls.is_some();

    if let Some(tls) = tls {
        let config = RustlsConfig::from_pem_file(&tls.cert, &tls.key)
            .await
            .map_err(|e| anyhow::anyhow!("failed to load TLS cert/key: {}", e))?;
        tokio::spawn(async move {
            if let Err(e) = axum_server::bind_rustls(addr, config)
                .handle(handle_spawn)
                .serve(app_make)
                .await
            {
                eprintln!("https server error: {}", e);
            }
        });
    } else {
        tokio::spawn(async move {
            if let Err(e) = axum_server::bind(addr)
                .handle(handle_spawn)
                .serve(app_make)
                .await
            {
                eprintln!("http server error: {}", e);
            }
        });
    }

    // Wait until the server is actually listening so we know the bound port.
    let listening_addr = handle
        .listening()
        .await
        .ok_or_else(|| anyhow::anyhow!("server failed to bind"))?;
    let bound_port = listening_addr.port();

    Ok(ServerHandle {
        handle,
        port: bound_port,
        https,
    })
}

pub fn tls_paths_exist(cert: &Path, key: &Path) -> bool {
    cert.exists() && key.exists()
}

async fn check_auth_async(
    state: &ServerState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> bool {
    let expected = state.token.lock().await.clone();
    if expected.is_empty() {
        return true;
    }
    if let Some(h) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(t) = h.strip_prefix("Bearer ") {
            if t == expected {
                return true;
            }
        }
    }
    if let Some(t) = query_token {
        if t == expected {
            return true;
        }
    }
    false
}

async fn auth_check(state: &ServerState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let expected = state.token.lock().await.clone();
    if expected.is_empty() {
        return Ok(());
    }
    if let Some(h) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(t) = h.strip_prefix("Bearer ") {
            if t == expected {
                return Ok(());
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn ping() -> &'static str {
    "pong"
}

async fn list_projects(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Project>>, StatusCode> {
    auth_check(&state, &headers).await?;
    state
        .db
        .list_projects()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
struct NewProject {
    name: String,
    path: String,
}

async fn add_project(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(body): Json<NewProject>,
) -> Result<Json<Project>, StatusCode> {
    auth_check(&state, &headers).await?;
    state
        .db
        .add_project(&body.name, &body.path)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn delete_project(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    state
        .db
        .delete_project(&id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn clear_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    state
        .db
        .clear_last_session(&id)
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
struct AttachmentResponse {
    ref_path: String,
}

async fn upload_attachment(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Bytes,
) -> Result<Json<AttachmentResponse>, StatusCode> {
    auth_check(&state, &headers).await?;
    let filename = headers
        .get("x-filename")
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // URL-decode minimally (spaces stored as %20)
            percent_decode(s)
        })
        .unwrap_or_else(|| "file".to_string());

    let project = state
        .db
        .get_project(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let ref_path =
        crate::save_attachment_to_project(&project.path, &filename, &body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(AttachmentResponse { ref_path }))
}

#[derive(Serialize)]
struct SttStatusResp {
    available: bool,
    model_ready: bool,
    model_path: String,
}

async fn stt_status(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<SttStatusResp>, StatusCode> {
    auth_check(&state, &headers).await?;
    Ok(Json(SttStatusResp {
        available: crate::stt::is_available(),
        model_ready: state.stt_model_path.exists(),
        model_path: state.stt_model_path.to_string_lossy().to_string(),
    }))
}

async fn stt_download(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    if state.stt_model_path.exists() {
        return Ok(StatusCode::OK);
    }
    crate::stt::download_model(&state.stt_model_path)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
struct TranscribeResp {
    text: String,
}

#[derive(Deserialize)]
struct TranscribeQuery {
    lang: Option<String>,
}

async fn claude_usage_route(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    auth_check(&state, &headers).await?;
    crate::usage::fetch_claude_usage()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn stt_transcribe(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<TranscribeQuery>,
    body: Bytes,
) -> Result<Json<TranscribeResp>, StatusCode> {
    auth_check(&state, &headers).await?;
    let model_path = state.stt_model_path.clone();
    let bytes = body.to_vec();
    let lang = params.lang.clone();
    let text = tokio::task::spawn_blocking(move || {
        crate::stt::transcribe_wav(&bytes, &model_path, lang.as_deref())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(TranscribeResp { text }))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Deserialize)]
struct HistoryQuery {
    before_ts: Option<String>,
    hours: Option<u32>,
}

async fn load_history(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<HistoryChunk>, StatusCode> {
    auth_check(&state, &headers).await?;
    let project = state
        .db
        .get_project(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let sid = match project.last_session_id {
        Some(s) => s,
        None => match history::latest_session_id(&project.path) {
            Some(s) => s,
            None => {
                return Ok(Json(HistoryChunk {
                    events: vec![],
                    oldest_ts: None,
                    has_more: false,
                    session_id: None,
                }))
            }
        },
    };
    history::load_session_chunk(
        &project.path,
        &sid,
        params.before_ts.as_deref(),
        params.hours.unwrap_or(2),
    )
    .map(Json)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
struct SendBody {
    prompt: String,
}

async fn send_message(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    Json(body): Json<SendBody>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    let project = state
        .db
        .get_project(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let resume = project.last_session_id.clone();
    let flags = crate::load_project_flags(&state.db, &id);
    let db = state.db.clone();
    let bus = state.bus.clone();
    let registry = state.registry.clone();
    let pid = id.clone();
    tokio::spawn(async move {
        match session::run_claude(
            bus,
            registry,
            pid.clone(),
            project.path,
            body.prompt,
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
    Ok(StatusCode::ACCEPTED)
}

async fn pause_message(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    let ok = state
        .registry
        .pause(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        let _ = state.bus.send(ClaudeEvent::Paused { project_id: id });
    }
    Ok(StatusCode::OK)
}

async fn resume_message(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    let ok = state
        .registry
        .resume(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        let _ = state.bus.send(ClaudeEvent::Resumed { project_id: id });
    }
    Ok(StatusCode::OK)
}

async fn cancel_message(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<StatusCode, StatusCode> {
    auth_check(&state, &headers).await?;
    state
        .registry
        .cancel(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct McpServer {
    name: String,
    status: String,
    ok: bool,
}

async fn git_diff(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<String, StatusCode> {
    auth_check(&state, &headers).await?;
    let project = state
        .db
        .get_project(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let output = tokio::process::Command::new("git")
        .arg("diff")
        .arg("HEAD")
        .current_dir(&project.path)
        .output()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn list_mcp_servers(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<Json<Vec<McpServer>>, StatusCode> {
    auth_check(&state, &headers).await?;
    let project = state
        .db
        .get_project(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let output = tokio::process::Command::new("claude")
        .arg("mcp")
        .arg("list")
        .current_dir(&project.path)
        .output()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut servers = Vec::new();
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
            servers.push(McpServer { name, status, ok });
        }
    }
    Ok(Json(servers))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let token = params.get("token").map(|s| s.as_str());
    if !check_auth_async(&state, &headers, token).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let bus = state.bus.clone();
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, bus)))
}

async fn handle_ws(socket: WebSocket, bus: EventBus) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = bus.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            let json = match serde_json::to_string(&ev) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub fn generate_token() -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
