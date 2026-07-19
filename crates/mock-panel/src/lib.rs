//! In-process mock of the Pterodactyl client API subset Wingman uses.
//!
//! REST: `GET /api/client` (paginated server list, `per_page` = 2 so the
//! pagination path is always exercised), `GET .../resources`,
//! `POST .../power` (with realistic delayed state transitions) and
//! `GET .../websocket` — all guarded by a Bearer key.
//!
//! Websocket: `/node/ws/{id}` speaks the Wings console protocol (auth frame,
//! `auth success`, `status`/`stats`/`console output` pushes, `send command`,
//! `send logs`, re-auth after `token expiring`). Response shapes mirror the
//! real panel so wingman-core is tested against realistic JSON.
//!
//! Run standalone with `cargo run -p mock-panel`.

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// The only API key the mock accepts.
pub const API_KEY: &str = "ptlc_mock_0000000000000000";

const PER_PAGE: usize = 2;
const TRANSITION_DELAY: Duration = Duration::from_millis(700);

/// Test hooks for the websocket side.
#[derive(Debug, Clone, Default)]
pub struct MockPanelOptions {
    /// Send a `token expiring` frame this long after each (re-)auth.
    pub token_expire_after: Option<Duration>,
    /// Hard-drop every websocket connection after this duration
    /// (exercises the client's reconnect path).
    pub drop_connection_after: Option<Duration>,
}

#[derive(Clone)]
struct AppState {
    servers: Arc<Mutex<HashMap<String, ServerRuntime>>>,
    options: Arc<MockPanelOptions>,
    addr: SocketAddr,
    backup_seq: Arc<std::sync::atomic::AtomicU64>,
}

struct ServerRuntime {
    state: &'static str,
    /// Serialized Wings frames, fanned out to every connected websocket.
    events: broadcast::Sender<String>,
    /// Virtual server filesystem: normalized relative path → contents.
    files: HashMap<String, Vec<u8>>,
    backups: Vec<MockBackup>,
}

#[derive(Clone)]
struct MockBackup {
    uuid: String,
    name: String,
    created_at: String,
    completed: bool,
}

impl MockBackup {
    fn to_json(&self) -> Value {
        json!({
            "uuid": self.uuid,
            "name": self.name,
            "is_successful": self.completed,
            "is_locked": false,
            "bytes": if self.completed { 1_048_576 } else { 0 },
            "created_at": self.created_at,
            "completed_at": if self.completed { Value::String(self.created_at.clone()) } else { Value::Null }
        })
    }
}

/// Backup slots per sample server (mirrors `feature_limits.backups`).
fn backup_limit(id: &str) -> usize {
    match id {
        "a1b2c3d4" => 3,
        "b2c3d4e5" => 1,
        "c3d4e5f6" => 2,
        _ => 0,
    }
}

impl AppState {
    fn new(addr: SocketAddr, options: MockPanelOptions) -> Self {
        let mut servers = HashMap::new();
        for (id, state) in [
            ("a1b2c3d4", "running"),
            ("b2c3d4e5", "starting"),
            ("c3d4e5f6", "offline"),
        ] {
            let (events, _) = broadcast::channel(64);
            servers.insert(
                id.to_string(),
                ServerRuntime {
                    state,
                    events,
                    files: HashMap::new(),
                    backups: Vec::new(),
                },
            );
        }
        Self {
            servers: Arc::new(Mutex::new(servers)),
            options: Arc::new(options),
            addr,
            backup_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn current_state(&self, id: &str) -> Option<&'static str> {
        self.servers.lock().unwrap().get(id).map(|rt| rt.state)
    }

    fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<String>> {
        self.servers
            .lock()
            .unwrap()
            .get(id)
            .map(|rt| rt.events.subscribe())
    }

    fn broadcast(&self, id: &str, frame: String) {
        if let Some(rt) = self.servers.lock().unwrap().get(id) {
            let _ = rt.events.send(frame);
        }
    }

    fn set_state(&self, id: &str, new_state: &'static str) {
        let mut servers = self.servers.lock().unwrap();
        if let Some(rt) = servers.get_mut(id) {
            rt.state = new_state;
            let _ = rt.events.send(frame("status", json!([new_state])));
            let _ = rt.events.send(stats_frame(new_state));
        }
    }

    fn put_file(&self, id: &str, path: String, bytes: Vec<u8>) {
        if let Some(rt) = self.servers.lock().unwrap().get_mut(id) {
            rt.files.insert(path, bytes);
        }
    }

    fn get_file(&self, id: &str, path: &str) -> Option<Vec<u8>> {
        self.servers
            .lock()
            .unwrap()
            .get(id)
            .and_then(|rt| rt.files.get(path).cloned())
    }

    /// Remove a path — an exact file match plus everything under it
    /// (directory semantics, like the panel's delete endpoint).
    fn remove_path(&self, id: &str, path: &str) {
        if let Some(rt) = self.servers.lock().unwrap().get_mut(id) {
            let prefix = format!("{path}/");
            rt.files
                .retain(|key, _| key != path && !key.starts_with(&prefix));
        }
    }

    fn list_files(&self, id: &str) -> Vec<String> {
        let mut files: Vec<String> = self
            .servers
            .lock()
            .unwrap()
            .get(id)
            .map(|rt| rt.files.keys().cloned().collect())
            .unwrap_or_default();
        files.sort();
        files
    }
}

/// Join a panel-style root (`/`, `/app`) with a relative name into the
/// normalized storage key used by the virtual filesystem (no leading slash).
fn join_remote(root: &str, name: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for part in root.split('/').chain(name.split('/')) {
        if !part.is_empty() && part != "." {
            segments.push(part);
        }
    }
    segments.join("/")
}

/// A running mock panel. The server task is aborted on drop.
pub struct MockPanel {
    addr: SocketAddr,
    handle: JoinHandle<()>,
    state: AppState,
}

impl MockPanel {
    /// Ephemeral localhost port, default options (for tests).
    pub async fn spawn() -> Self {
        Self::spawn_with(MockPanelOptions::default()).await
    }

    pub async fn spawn_with(options: MockPanelOptions) -> Self {
        Self::spawn_on(SocketAddr::from(([127, 0, 0, 1], 0)), options).await
    }

    pub async fn spawn_on(addr: SocketAddr, options: MockPanelOptions) -> Self {
        let listener = TcpListener::bind(addr).await.expect("bind mock panel");
        let addr = listener.local_addr().expect("local addr");
        let state = AppState::new(addr, options);
        let router_state = state.clone();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router(router_state))
                .await
                .expect("serve mock panel");
        });
        Self {
            addr,
            handle,
            state,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Test inspection: all files on the server's virtual filesystem, sorted.
    pub fn server_files(&self, id: &str) -> Vec<String> {
        self.state.list_files(id)
    }

    /// Test inspection: contents of one file on the virtual filesystem.
    pub fn file_contents(&self, id: &str, path: &str) -> Option<Vec<u8>> {
        self.state.get_file(id, path)
    }
}

impl Drop for MockPanel {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/client", get(list_servers))
        .route("/api/client/servers/{id}/resources", get(server_resources))
        .route("/api/client/servers/{id}/power", post(set_power))
        .route("/api/client/servers/{id}/websocket", get(websocket_details))
        .route("/api/client/servers/{id}", get(server_details))
        .route(
            "/api/client/servers/{id}/backups",
            get(list_backups).post(create_backup),
        )
        .route(
            "/api/client/servers/{id}/backups/{uuid}",
            get(backup_details).delete(delete_backup),
        )
        .route("/api/client/servers/{id}/files/upload", get(upload_url))
        .route(
            "/api/client/servers/{id}/files/decompress",
            post(decompress_file),
        )
        .route("/api/client/servers/{id}/files/delete", post(delete_files))
        .route(
            "/api/client/servers/{id}/files/create-folder",
            post(create_folder),
        )
        .route("/node/ws/{id}", get(node_ws))
        .route("/node/upload/{id}", post(node_upload))
        .with_state(state)
}

fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("Bearer {API_KEY}"))
        .unwrap_or(false)
}

fn error_response(status: StatusCode, code: &str, detail: &str) -> Response {
    let body = json!({
        "errors": [{ "code": code, "status": status.as_u16().to_string(), "detail": detail }]
    });
    (status, Json(body)).into_response()
}

fn unauthorized() -> Response {
    error_response(
        StatusCode::UNAUTHORIZED,
        "InvalidCredentialsException",
        "The credentials provided were invalid.",
    )
}

fn not_found() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "NotFoundHttpException",
        "The requested resource could not be found on the server.",
    )
}

// ---------------------------------------------------------------------------
// REST handlers
// ---------------------------------------------------------------------------

async fn list_servers(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let servers = sample_servers();
    let total = servers.len();
    let total_pages = total.div_ceil(PER_PAGE).max(1);
    let page: usize = query
        .get("page")
        .and_then(|p| p.parse().ok())
        .filter(|p| *p >= 1)
        .unwrap_or(1);
    let page_items: Vec<Value> = servers
        .into_iter()
        .skip((page - 1) * PER_PAGE)
        .take(PER_PAGE)
        .map(|attributes| json!({ "object": "server", "attributes": attributes }))
        .collect();
    let body = json!({
        "object": "list",
        "data": page_items,
        "meta": {
            "pagination": {
                "total": total,
                "count": page_items.len(),
                "per_page": PER_PAGE,
                "current_page": page,
                "total_pages": total_pages,
                "links": {}
            }
        }
    });
    Json(body).into_response()
}

async fn server_resources(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let Some(state) = app.current_state(&id) else {
        return not_found();
    };
    let stats = stats_payload(state);
    Json(json!({
        "object": "stats",
        "attributes": {
            "current_state": state,
            "is_suspended": false,
            "resources": {
                "memory_bytes": stats["memory_bytes"],
                "cpu_absolute": stats["cpu_absolute"],
                "disk_bytes": stats["disk_bytes"],
                "network_rx_bytes": stats["network"]["rx_bytes"],
                "network_tx_bytes": stats["network"]["tx_bytes"],
                "uptime": stats["uptime"]
            }
        }
    }))
    .into_response()
}

async fn set_power(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    let signal = body.get("signal").and_then(Value::as_str).unwrap_or("");
    if !["start", "stop", "restart", "kill"].contains(&signal) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ValidationException",
            "The selected signal is invalid.",
        );
    }
    tokio::spawn(power_transition(app.clone(), id, signal.to_string()));
    StatusCode::NO_CONTENT.into_response()
}

/// State transitions like a real server: intermediate states first, the
/// final state after a short delay. Status + stats frames go out per step.
async fn power_transition(app: AppState, id: String, signal: String) {
    match signal.as_str() {
        "start" => {
            app.set_state(&id, "starting");
            tokio::time::sleep(TRANSITION_DELAY).await;
            app.set_state(&id, "running");
        }
        "stop" => {
            app.set_state(&id, "stopping");
            tokio::time::sleep(TRANSITION_DELAY).await;
            app.set_state(&id, "offline");
        }
        "restart" => {
            app.set_state(&id, "stopping");
            tokio::time::sleep(TRANSITION_DELAY).await;
            app.set_state(&id, "starting");
            tokio::time::sleep(TRANSITION_DELAY).await;
            app.set_state(&id, "running");
        }
        "kill" => app.set_state(&id, "offline"),
        _ => {}
    }
}

async fn websocket_details(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    Json(json!({
        "data": {
            "token": format!("mock_wt_{id}"),
            "socket": format!("ws://{}/node/ws/{id}", app.addr)
        }
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Server details & backups
// ---------------------------------------------------------------------------

async fn server_details(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    let Some(attributes) = sample_servers()
        .into_iter()
        .find(|s| s["identifier"] == id.as_str())
    else {
        return not_found();
    };
    Json(json!({ "object": "server", "attributes": attributes })).into_response()
}

async fn list_backups(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let servers = app.servers.lock().unwrap();
    let Some(rt) = servers.get(&id) else {
        return not_found();
    };
    let data: Vec<Value> = rt
        .backups
        .iter()
        .map(|b| json!({ "object": "backup", "attributes": b.to_json() }))
        .collect();
    let total = data.len();
    Json(json!({
        "object": "list",
        "data": data,
        "meta": {
            "backup_count": total,
            "pagination": {
                "total": total, "count": total, "per_page": 50,
                "current_page": 1, "total_pages": 1, "links": {}
            }
        }
    }))
    .into_response()
}

/// Create a backup; completes asynchronously after a short delay, like a
/// real node. Rejects when the server's backup limit is reached.
async fn create_backup(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let seq = app
        .backup_seq
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let backup = {
        let mut servers = app.servers.lock().unwrap();
        let Some(rt) = servers.get_mut(&id) else {
            return not_found();
        };
        if rt.backups.len() >= backup_limit(&id) {
            return error_response(
                StatusCode::BAD_REQUEST,
                "TooManyBackupsException",
                "Cannot create a new backup, this server has reached its limit.",
            );
        }
        let backup = MockBackup {
            uuid: format!("backup-{seq}"),
            name: body
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())
                .unwrap_or("unnamed")
                .to_string(),
            created_at: format!("2026-07-19T00:{:02}:{:02}Z", (seq / 60) % 60, seq % 60),
            completed: false,
        };
        rt.backups.push(backup.clone());
        backup
    };

    // Complete the backup shortly after, like a real node.
    let complete_app = app.clone();
    let complete_id = id.clone();
    let complete_uuid = backup.uuid.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut servers = complete_app.servers.lock().unwrap();
        if let Some(rt) = servers.get_mut(&complete_id) {
            if let Some(b) = rt.backups.iter_mut().find(|b| b.uuid == complete_uuid) {
                b.completed = true;
            }
        }
    });

    Json(json!({ "object": "backup", "attributes": backup.to_json() })).into_response()
}

async fn backup_details(
    State(app): State<AppState>,
    Path((id, uuid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let servers = app.servers.lock().unwrap();
    let Some(backup) = servers
        .get(&id)
        .and_then(|rt| rt.backups.iter().find(|b| b.uuid == uuid))
    else {
        return not_found();
    };
    Json(json!({ "object": "backup", "attributes": backup.to_json() })).into_response()
}

async fn delete_backup(
    State(app): State<AppState>,
    Path((id, uuid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let mut servers = app.servers.lock().unwrap();
    let Some(rt) = servers.get_mut(&id) else {
        return not_found();
    };
    let before = rt.backups.len();
    rt.backups.retain(|b| b.uuid != uuid);
    if rt.backups.len() == before {
        return not_found();
    }
    StatusCode::NO_CONTENT.into_response()
}

// ---------------------------------------------------------------------------
// File API (virtual filesystem)
// ---------------------------------------------------------------------------

async fn upload_url(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    Json(json!({
        "object": "signed_url",
        "attributes": {
            "url": format!("http://{}/node/upload/{id}?token=mock", app.addr)
        }
    }))
    .into_response()
}

/// The node-side upload target the signed URL points at. Like Wings, it
/// takes multipart `files` fields and a `directory` query parameter; the
/// signed token replaces Bearer auth.
async fn node_upload(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Response {
    if app.current_state(&id).is_none() {
        return not_found();
    }
    let directory = query
        .get("directory")
        .cloned()
        .unwrap_or_else(|| "/".into());
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(file_name) = field.file_name().map(str::to_string) else {
            continue;
        };
        match field.bytes().await {
            Ok(bytes) => app.put_file(&id, join_remote(&directory, &file_name), bytes.to_vec()),
            Err(_) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "DaemonException",
                    "Failed to read the uploaded file.",
                )
            }
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn decompress_file(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    let root = body.get("root").and_then(Value::as_str).unwrap_or("/");
    let file = body.get("file").and_then(Value::as_str).unwrap_or("");
    let archive_path = join_remote(root, file);
    let Some(bytes) = app.get_file(&id, &archive_path) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "DaemonException",
            "The requested archive was not found.",
        );
    };
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "DaemonException",
            "The archive could not be read.",
        );
    };
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let mut contents = Vec::new();
        if std::io::Read::read_to_end(&mut entry, &mut contents).is_ok() {
            app.put_file(&id, join_remote(root, &name), contents);
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn delete_files(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    let root = body.get("root").and_then(Value::as_str).unwrap_or("/");
    let files = body
        .get("files")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for file in files {
        app.remove_path(&id, &join_remote(root, &file));
    }
    StatusCode::NO_CONTENT.into_response()
}

/// The virtual filesystem has no directory entries — creating a folder is a
/// no-op that only validates the request, like a very forgiving panel.
async fn create_folder(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(_body): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    if app.current_state(&id).is_none() {
        return not_found();
    }
    StatusCode::NO_CONTENT.into_response()
}

// ---------------------------------------------------------------------------
// Websocket (Wings protocol)
// ---------------------------------------------------------------------------

async fn node_ws(
    State(app): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_node_ws(socket, app, id))
}

async fn handle_node_ws(socket: WebSocket, app: AppState, id: String) {
    let Some(mut events_rx) = app.subscribe(&id) else {
        return;
    };
    let (mut sink, mut stream) = socket.split();

    if !wait_for_auth(&mut stream).await {
        return;
    }
    let send = |s: String| WsMessage::Text(s.into());
    if sink
        .send(send(frame("auth success", json!([]))))
        .await
        .is_err()
    {
        return;
    }
    let Some(state) = app.current_state(&id) else {
        return;
    };
    let _ = sink.send(send(frame("status", json!([state])))).await;
    let _ = sink.send(send(stats_frame(state))).await;

    let mut stats_tick = tokio::time::interval(Duration::from_secs(1));
    stats_tick.tick().await; // Consume the immediate first tick.
    let mut expire_deadline = deadline(app.options.token_expire_after);
    let drop_deadline = deadline(app.options.drop_connection_after);

    loop {
        tokio::select! {
            _ = stats_tick.tick() => {
                let Some(state) = app.current_state(&id) else { return };
                if sink.send(send(stats_frame(state))).await.is_err() {
                    return;
                }
            }
            broadcast_frame = events_rx.recv() => {
                // Lagged receivers just skip; the sender lives in AppState
                // and is never dropped, so Closed cannot occur.
                if let Ok(payload) = broadcast_frame {
                    if sink.send(send(payload)).await.is_err() {
                        return;
                    }
                }
            }
            message = stream.next() => {
                let Some(Ok(message)) = message else { return };
                let WsMessage::Text(text) = message else {
                    if matches!(message, WsMessage::Close(_)) { return; }
                    continue;
                };
                let Some((event, args)) = parse_frame(text.as_str()) else { continue };
                match event.as_str() {
                    // Re-auth after "token expiring".
                    "auth" => {
                        if sink.send(send(frame("auth success", json!([])))).await.is_err() {
                            return;
                        }
                        expire_deadline = deadline(app.options.token_expire_after);
                    }
                    "send command" => {
                        if let Some(command) = args.first().and_then(Value::as_str) {
                            app.broadcast(&id, frame("console output", json!([format!("> {command}")])));
                            app.broadcast(&id, frame(
                                "console output",
                                json!([format!("\u{1b}[32m[mock]\u{1b}[0m Executed '{command}'")]),
                            ));
                        }
                    }
                    "send logs" => {
                        for line in boot_lines() {
                            if sink.send(send(frame("console output", json!([line])))).await.is_err() {
                                return;
                            }
                        }
                    }
                    "send stats" => {
                        let Some(state) = app.current_state(&id) else { return };
                        if sink.send(send(stats_frame(state))).await.is_err() {
                            return;
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep_until(expire_deadline) => {
                if sink.send(send(frame("token expiring", json!([])))).await.is_err() {
                    return;
                }
                expire_deadline = deadline(app.options.token_expire_after);
            }
            _ = tokio::time::sleep_until(drop_deadline) => {
                // Hard drop, no close frame — like a node restart.
                return;
            }
        }
    }
}

async fn wait_for_auth(stream: &mut SplitStream<WebSocket>) -> bool {
    while let Some(Ok(message)) = stream.next().await {
        if let WsMessage::Text(text) = message {
            if let Some((event, args)) = parse_frame(text.as_str()) {
                if event == "auth" {
                    return args
                        .first()
                        .and_then(Value::as_str)
                        .map(|token| token.starts_with("mock_wt_"))
                        .unwrap_or(false);
                }
            }
        }
    }
    false
}

fn parse_frame(text: &str) -> Option<(String, Vec<Value>)> {
    #[derive(serde::Deserialize)]
    struct WireFrame {
        event: String,
        #[serde(default)]
        args: Vec<Value>,
    }
    serde_json::from_str::<WireFrame>(text)
        .ok()
        .map(|f| (f.event, f.args))
}

/// An absolute deadline; `None` means "practically never".
fn deadline(after: Option<Duration>) -> Instant {
    Instant::now() + after.unwrap_or(Duration::from_secs(60 * 60 * 24 * 365))
}

fn frame(event: &str, args: Value) -> String {
    json!({ "event": event, "args": args }).to_string()
}

/// Wings double-encodes stats: args[0] is a JSON string, not an object.
fn stats_frame(state: &str) -> String {
    frame("stats", json!([stats_payload(state).to_string()]))
}

fn stats_payload(state: &str) -> Value {
    let (memory, cpu, uptime) = match state {
        "running" => (1_610_612_736u64, 87.5, 86_400_000u64),
        "starting" | "stopping" => (268_435_456, 12.0, 4_000),
        _ => (0, 0.0, 0),
    };
    json!({
        "memory_bytes": memory,
        "memory_limit_bytes": 4_294_967_296u64,
        "cpu_absolute": cpu,
        "disk_bytes": 2_147_483_648u64,
        "network": { "rx_bytes": 694_220, "tx_bytes": 337_090 },
        "uptime": uptime,
        "state": state
    })
}

/// Console backlog replayed on "send logs". Includes ANSI colors so the
/// frontend's escape-sequence stripping is exercised against real-ish data.
fn boot_lines() -> Vec<String> {
    vec![
        "\u{1b}[33m[mock]\u{1b}[0m Container booted".to_string(),
        "\u{1b}[32m[mock]\u{1b}[0m Server marked as ready".to_string(),
    ]
}

fn sample_servers() -> Vec<Value> {
    vec![
        server(
            "a1b2c3d4",
            "Survival SMP",
            "Main Minecraft server",
            4096,
            10_240,
            200,
            3,
        ),
        server(
            "b2c3d4e5",
            "Discord Bot",
            "Moderation bot",
            512,
            1_024,
            100,
            1,
        ),
        server("c3d4e5f6", "Factorio", "Weekend factory", 2048, 5_120, 0, 2),
    ]
}

fn server(
    identifier: &str,
    name: &str,
    description: &str,
    memory: u64,
    disk: u64,
    cpu: u64,
    backups: u64,
) -> Value {
    json!({
        "server_owner": true,
        "identifier": identifier,
        "internal_id": 1,
        "uuid": format!("{identifier}-aaaa-bbbb-cccc-dddddddddddd"),
        "name": name,
        "node": "Node 01",
        "description": description,
        "limits": {
            "memory": memory,
            "swap": 0,
            "disk": disk,
            "io": 500,
            "cpu": cpu
        },
        "invocation": "java -Xms128M -jar server.jar",
        "docker_image": "ghcr.io/pterodactyl/yolks:java_21",
        "sftp_details": { "ip": "node01.example.com", "port": 2022 },
        "feature_limits": {
            "databases": 1,
            "allocations": 1,
            "backups": backups
        },
        "status": null,
        "is_suspended": false,
        "is_installing": false,
        "is_transferring": false
    })
}
