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
use axum::extract::{Path, Query, State};
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
}

struct ServerRuntime {
    state: &'static str,
    /// Serialized Wings frames, fanned out to every connected websocket.
    events: broadcast::Sender<String>,
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
            servers.insert(id.to_string(), ServerRuntime { state, events });
        }
        Self {
            servers: Arc::new(Mutex::new(servers)),
            options: Arc::new(options),
            addr,
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
}

/// A running mock panel. The server task is aborted on drop.
pub struct MockPanel {
    addr: SocketAddr,
    handle: JoinHandle<()>,
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
        let handle = tokio::spawn(async move {
            axum::serve(listener, router(state))
                .await
                .expect("serve mock panel");
        });
        Self { addr, handle }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
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
        .route("/node/ws/{id}", get(node_ws))
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
