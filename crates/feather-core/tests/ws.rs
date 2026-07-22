//! Integration tests: ServerSocket (Wings websocket client) against the mock
//! panel's websocket implementation.

use feather_core::models::{PowerSignal, PowerState};
use feather_core::ws::{Outgoing, ServerEvent, ServerSocket};
use feather_core::PanelClient;
use mock_panel::{MockPanel, MockPanelOptions, API_KEY};
use std::time::Duration;
use tokio::time::timeout;

async fn next(socket: &mut ServerSocket) -> ServerEvent {
    timeout(Duration::from_secs(10), socket.events.recv())
        .await
        .expect("timed out waiting for server event")
        .expect("event channel closed")
}

/// Skip events until `predicate` matches; panics on timeout via `next`.
async fn wait_for(
    socket: &mut ServerSocket,
    predicate: impl Fn(&ServerEvent) -> bool,
) -> ServerEvent {
    loop {
        let event = next(socket).await;
        if predicate(&event) {
            return event;
        }
    }
}

fn connect(panel: &MockPanel, identifier: &str) -> ServerSocket {
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    ServerSocket::spawn(client, identifier.to_string())
}

#[tokio::test]
async fn authenticates_and_receives_status_and_stats() {
    let panel = MockPanel::spawn().await;
    let mut socket = connect(&panel, "a1b2c3d4");

    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;
    wait_for(&mut socket, |e| {
        matches!(e, ServerEvent::Status(PowerState::Running))
    })
    .await;
    let stats = wait_for(&mut socket, |e| matches!(e, ServerEvent::Stats(_))).await;
    let ServerEvent::Stats(stats) = stats else {
        unreachable!()
    };
    assert_eq!(stats.state, PowerState::Running);
    assert!(stats.memory_bytes > 0);
    assert!(stats.cpu_absolute > 0.0);
    assert!(stats.memory_limit_bytes > 0);
}

#[tokio::test]
async fn replays_logs_and_echoes_console_commands() {
    let panel = MockPanel::spawn().await;
    let mut socket = connect(&panel, "a1b2c3d4");
    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;

    // The client requests the backlog right after auth.
    wait_for(
        &mut socket,
        |e| matches!(e, ServerEvent::Console(line) if line.contains("Container booted")),
    )
    .await;

    socket
        .outgoing
        .send(Outgoing::Command("say hello".into()))
        .await
        .unwrap();
    wait_for(
        &mut socket,
        |e| matches!(e, ServerEvent::Console(line) if line.contains("say hello")),
    )
    .await;
}

#[tokio::test]
async fn refreshes_expiring_tokens_without_disconnecting() {
    let panel = MockPanel::spawn_with(MockPanelOptions {
        token_expire_after: Some(Duration::from_millis(200)),
        ..Default::default()
    })
    .await;
    let mut socket = connect(&panel, "a1b2c3d4");
    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;

    // The mock announces expiry after 200 ms; the client must fetch a fresh
    // token and re-auth on the same connection — visible as a second
    // Connected without any Disconnected in between.
    loop {
        match next(&mut socket).await {
            ServerEvent::Connected => break,
            ServerEvent::Disconnected { reason } => {
                panic!("connection dropped instead of refreshing the token: {reason}")
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn reconnects_after_connection_drop() {
    let panel = MockPanel::spawn_with(MockPanelOptions {
        drop_connection_after: Some(Duration::from_millis(300)),
        ..Default::default()
    })
    .await;
    let mut socket = connect(&panel, "a1b2c3d4");

    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;
    wait_for(&mut socket, |e| {
        matches!(e, ServerEvent::Disconnected { .. })
    })
    .await;
    // Backoff starts at 1 s, so the reconnect lands within the 10 s budget.
    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;
}

#[tokio::test]
async fn power_transitions_arrive_as_status_events() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    let mut socket = connect(&panel, "c3d4e5f6");
    wait_for(&mut socket, |e| matches!(e, ServerEvent::Connected)).await;

    client
        .set_power("c3d4e5f6", PowerSignal::Start)
        .await
        .unwrap();

    wait_for(&mut socket, |e| {
        matches!(e, ServerEvent::Status(PowerState::Starting))
    })
    .await;
    wait_for(&mut socket, |e| {
        matches!(e, ServerEvent::Status(PowerState::Running))
    })
    .await;
}

#[tokio::test]
async fn invalid_identifier_disconnects_fatally() {
    let panel = MockPanel::spawn().await;
    let mut socket = connect(&panel, "../etc");

    let event = next(&mut socket).await;
    assert!(
        matches!(event, ServerEvent::Disconnected { .. }),
        "expected fatal disconnect, got {event:?}"
    );
    // The task must end (no endless retry loop): the channel closes.
    let closed = timeout(Duration::from_secs(5), socket.events.recv())
        .await
        .expect("timed out waiting for channel close");
    assert!(closed.is_none(), "socket kept retrying a fatal error");
}
