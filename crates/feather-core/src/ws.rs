//! Websocket connection to a server's Wings daemon: live console, status and
//! resource stats — the same channel the panel's own console uses.
//!
//! Protocol: fetch short-lived credentials via the client API, connect to the
//! node, authenticate with `{"event":"auth","args":[token]}`. Wings announces
//! expiry with `token expiring`, after which a fresh token has to be fetched
//! and re-sent — the connection itself stays open. [`ServerSocket`] wraps all
//! of this in a supervised task with reconnect + backoff.

use crate::api::PanelClient;
use crate::error::Error;
use crate::models::PowerState;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::ORIGIN;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Live resource snapshot pushed by Wings in `stats` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsStats {
    pub memory_bytes: u64,
    #[serde(default)]
    pub memory_limit_bytes: u64,
    pub cpu_absolute: f64,
    #[serde(default)]
    pub disk_bytes: u64,
    #[serde(default)]
    pub uptime: u64,
    pub state: PowerState,
    #[serde(default)]
    pub network: WsNetwork,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WsNetwork {
    #[serde(default)]
    pub rx_bytes: u64,
    #[serde(default)]
    pub tx_bytes: u64,
}

/// Events emitted to the consumer. Serialized as `{"type":…,"data":…}` so the
/// Tauri shell can forward them to the frontend as-is.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Authenticated (also emitted after a successful token refresh/reconnect).
    Connected,
    Status(PowerState),
    Stats(WsStats),
    Console(String),
    /// Connection lost. The socket keeps reconnecting with backoff unless the
    /// cause is fatal (revoked key, invalid identifier).
    Disconnected {
        reason: String,
    },
}

/// Requests the consumer can send over the socket.
#[derive(Debug, Clone)]
pub enum Outgoing {
    /// Run a command on the server console.
    Command(String),
    /// Ask Wings to replay the recent console backlog.
    RequestLogs,
    /// Ask Wings for an immediate stats push.
    RequestStats,
}

/// Handle to a supervised websocket task. Dropping `outgoing` (or the whole
/// handle) shuts the connection down; `events` closing means the task ended.
pub struct ServerSocket {
    pub events: mpsc::Receiver<ServerEvent>,
    pub outgoing: mpsc::Sender<Outgoing>,
}

impl ServerSocket {
    /// Spawn the connection task on the current tokio runtime.
    pub fn spawn(client: PanelClient, identifier: String) -> ServerSocket {
        let (event_tx, events) = mpsc::channel(256);
        let (outgoing, outgoing_rx) = mpsc::channel(64);
        tokio::spawn(run(client, identifier, event_tx, outgoing_rx));
        ServerSocket { events, outgoing }
    }
}

enum SessionEnd {
    /// Consumer dropped the outgoing sender — stop for good.
    Shutdown,
    /// Connection failed or closed — reconnect.
    Lost(String),
}

async fn run(
    client: PanelClient,
    identifier: String,
    events: mpsc::Sender<ServerEvent>,
    mut outgoing: mpsc::Receiver<Outgoing>,
) {
    let mut backoff = INITIAL_BACKOFF;
    loop {
        let mut authenticated = false;
        let end = session(
            &client,
            &identifier,
            &events,
            &mut outgoing,
            &mut authenticated,
        )
        .await;
        let reason = match end {
            Ok(SessionEnd::Shutdown) => return,
            Ok(SessionEnd::Lost(reason)) => reason,
            Err(err @ (Error::Unauthorized { .. } | Error::InvalidServerIdentifier(_))) => {
                // Fatal: retrying cannot succeed until the user intervenes.
                let _ = events
                    .send(ServerEvent::Disconnected {
                        reason: err.to_string(),
                    })
                    .await;
                return;
            }
            Err(err) => err.to_string(),
        };
        if events
            .send(ServerEvent::Disconnected { reason })
            .await
            .is_err()
        {
            return;
        }
        if authenticated {
            backoff = INITIAL_BACKOFF;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// One connection lifetime: connect, authenticate, pump events until the
/// connection drops or the consumer shuts down.
async fn session(
    client: &PanelClient,
    identifier: &str,
    events: &mpsc::Sender<ServerEvent>,
    outgoing: &mut mpsc::Receiver<Outgoing>,
    authenticated: &mut bool,
) -> Result<SessionEnd, Error> {
    let details = client.websocket_details(identifier).await?;
    let mut request = details
        .socket
        .as_str()
        .into_client_request()
        .map_err(|e| Error::Websocket(e.to_string()))?;
    request.headers_mut().insert(
        ORIGIN,
        HeaderValue::from_str(&client.origin()).map_err(|e| Error::Websocket(e.to_string()))?,
    );
    let (stream, _) = connect_async(request)
        .await
        .map_err(|e| Error::Websocket(e.to_string()))?;
    let (mut sink, mut source) = stream.split();

    send_frame(&mut sink, "auth", json!([details.token])).await?;

    loop {
        tokio::select! {
            message = source.next() => {
                let Some(Ok(message)) = message else {
                    return Ok(SessionEnd::Lost("connection closed".into()));
                };
                match message {
                    Message::Text(text) => {
                        if let Some(end) = handle_frame(
                            client, identifier, events, &mut sink, text.as_str(), authenticated,
                        ).await? {
                            return Ok(end);
                        }
                    }
                    Message::Ping(payload) => {
                        sink.send(Message::Pong(payload))
                            .await
                            .map_err(|e| Error::Websocket(e.to_string()))?;
                    }
                    Message::Close(_) => return Ok(SessionEnd::Lost("closed by server".into())),
                    _ => {}
                }
            }
            request = outgoing.recv() => {
                let Some(request) = request else {
                    let _ = sink.close().await;
                    return Ok(SessionEnd::Shutdown);
                };
                let (event, args) = match &request {
                    Outgoing::Command(command) => ("send command", json!([command])),
                    Outgoing::RequestLogs => ("send logs", json!([null])),
                    Outgoing::RequestStats => ("send stats", json!([null])),
                };
                send_frame(&mut sink, event, args).await?;
            }
        }
    }
}

/// Handle one Wings frame. Returns `Some(end)` when the session must end.
async fn handle_frame(
    client: &PanelClient,
    identifier: &str,
    events: &mpsc::Sender<ServerEvent>,
    sink: &mut WsSink,
    text: &str,
    authenticated: &mut bool,
) -> Result<Option<SessionEnd>, Error> {
    #[derive(Deserialize)]
    struct Frame {
        event: String,
        #[serde(default)]
        args: Vec<Value>,
    }

    let Ok(frame) = serde_json::from_str::<Frame>(text) else {
        return Ok(None); // Unknown frame shape — ignore.
    };
    let first_arg = || frame.args.first().and_then(Value::as_str);

    match frame.event.as_str() {
        "auth success" => {
            *authenticated = true;
            if events.send(ServerEvent::Connected).await.is_err() {
                return Ok(Some(SessionEnd::Shutdown));
            }
            // Prime the session: console backlog + an immediate stats push.
            send_frame(sink, "send logs", json!([null])).await?;
            send_frame(sink, "send stats", json!([null])).await?;
        }
        "token expiring" => {
            let details = client.websocket_details(identifier).await?;
            send_frame(sink, "auth", json!([details.token])).await?;
        }
        "token expired" | "jwt error" => {
            return Ok(Some(SessionEnd::Lost("websocket token expired".into())));
        }
        "status" => {
            if let Some(state) = first_arg().and_then(PowerState::parse_wings) {
                if events.send(ServerEvent::Status(state)).await.is_err() {
                    return Ok(Some(SessionEnd::Shutdown));
                }
            }
        }
        // Wings double-encodes stats: args[0] is a JSON string.
        "stats" => {
            if let Some(stats) = first_arg().and_then(|raw| serde_json::from_str(raw).ok()) {
                if events.send(ServerEvent::Stats(stats)).await.is_err() {
                    return Ok(Some(SessionEnd::Shutdown));
                }
            }
        }
        "console output" | "install output" | "daemon message" => {
            if let Some(line) = first_arg() {
                if events
                    .send(ServerEvent::Console(line.to_string()))
                    .await
                    .is_err()
                {
                    return Ok(Some(SessionEnd::Shutdown));
                }
            }
        }
        _ => {}
    }
    Ok(None)
}

async fn send_frame(sink: &mut WsSink, event: &str, args: Value) -> Result<(), Error> {
    let payload = json!({ "event": event, "args": args }).to_string();
    sink.send(Message::Text(payload.into()))
        .await
        .map_err(|e| Error::Websocket(e.to_string()))
}
