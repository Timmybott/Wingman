//! In-process mock of the Pterodactyl client API subset Wingman uses.
//!
//! Serves `GET /api/client` (paginated server list, `per_page` = 2 so the
//! pagination path is always exercised) and
//! `GET /api/client/servers/{id}/resources`, guarded by a Bearer key.
//! Response shapes mirror the real panel so wingman-core's models are tested
//! against realistic JSON. Run standalone with `cargo run -p mock-panel`.

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// The only API key the mock accepts.
pub const API_KEY: &str = "ptlc_mock_0000000000000000";

const PER_PAGE: usize = 2;

/// A running mock panel. The server task is aborted on drop.
pub struct MockPanel {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl MockPanel {
    /// Bind to an ephemeral localhost port (for tests).
    pub async fn spawn() -> Self {
        Self::spawn_on(SocketAddr::from(([127, 0, 0, 1], 0))).await
    }

    pub async fn spawn_on(addr: SocketAddr) -> Self {
        let listener = TcpListener::bind(addr).await.expect("bind mock panel");
        let addr = listener.local_addr().expect("local addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, router())
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

pub fn router() -> Router {
    Router::new()
        .route("/api/client", get(list_servers))
        .route("/api/client/servers/{id}/resources", get(server_resources))
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

async fn server_resources(headers: HeaderMap, Path(id): Path<String>) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let stats = match id.as_str() {
        "a1b2c3d4" => stats("running", 1_610_612_736, 87.5, 2_147_483_648, 86_400_000),
        "b2c3d4e5" => stats("starting", 268_435_456, 12.0, 524_288_000, 4_000),
        "c3d4e5f6" => stats("offline", 0, 0.0, 1_073_741_824, 0),
        _ => {
            return error_response(
                StatusCode::NOT_FOUND,
                "NotFoundHttpException",
                "The requested resource could not be found on the server.",
            )
        }
    };
    Json(json!({ "object": "stats", "attributes": stats })).into_response()
}

fn stats(state: &str, memory: u64, cpu: f64, disk: u64, uptime: u64) -> Value {
    json!({
        "current_state": state,
        "is_suspended": false,
        "resources": {
            "memory_bytes": memory,
            "cpu_absolute": cpu,
            "disk_bytes": disk,
            "network_rx_bytes": 694_220,
            "network_tx_bytes": 337_090,
            "uptime": uptime
        }
    })
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
