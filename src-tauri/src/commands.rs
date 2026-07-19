//! IPC commands exposed to the frontend. Errors cross the IPC boundary as
//! strings, so every command maps core errors with `to_string()`.

use crate::secrets;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use wingman_core::models::{PowerSignal, Server, ServerStats};
use wingman_core::ws::Outgoing;
use wingman_core::{normalize_base_url, PanelClient, PanelConfig, ServerSocket};

type CmdResult<T> = Result<T, String>;

/// A subscribed server: sender half of the websocket task plus the forwarder
/// that turns core events into Tauri events for the frontend.
pub struct SocketHandle {
    outgoing: mpsc::Sender<Outgoing>,
    forwarder: tauri::async_runtime::JoinHandle<()>,
}

fn client_for(state: &AppState) -> CmdResult<PanelClient> {
    let panels = state.store.load_panels().map_err(|e| e.to_string())?;
    let panel = panels
        .into_iter()
        .next()
        .ok_or_else(|| "no panel configured".to_string())?;
    let api_key = secrets::get_api_key(&panel.id)?;
    PanelClient::new(&panel.base_url, &api_key).map_err(|e| e.to_string())
}

/// The configured panel, if any (v1 supports exactly one).
#[tauri::command]
pub fn get_panel(state: State<'_, AppState>) -> CmdResult<Option<PanelConfig>> {
    Ok(state
        .store
        .load_panels()
        .map_err(|e| e.to_string())?
        .into_iter()
        .next())
}

/// Dry-run credentials check; returns the number of visible servers.
#[tauri::command]
pub async fn test_connection(base_url: String, api_key: String) -> CmdResult<usize> {
    let client = PanelClient::new(&base_url, &api_key).map_err(|e| e.to_string())?;
    let servers = client.list_servers().await.map_err(|e| e.to_string())?;
    Ok(servers.len())
}

/// Verify the credentials, then persist: URL in the config file, API key in
/// the OS keychain.
#[tauri::command]
pub async fn save_panel(
    state: State<'_, AppState>,
    name: String,
    base_url: String,
    api_key: String,
) -> CmdResult<PanelConfig> {
    let url = normalize_base_url(&base_url).map_err(|e| e.to_string())?;
    let client = PanelClient::new(url.as_str(), &api_key).map_err(|e| e.to_string())?;
    client.list_servers().await.map_err(|e| e.to_string())?;

    let display_name = match name.trim() {
        "" => url.host_str().unwrap_or("Panel").to_string(),
        trimmed => trimmed.to_string(),
    };
    let panel = PanelConfig::new(display_name, url.to_string());
    secrets::set_api_key(&panel.id, api_key.trim())?;
    state
        .store
        .save_panels(std::slice::from_ref(&panel))
        .map_err(|e| e.to_string())?;
    Ok(panel)
}

/// Remove the panel connection, its keychain entry and all live sockets.
#[tauri::command]
pub async fn remove_panel(state: State<'_, AppState>) -> CmdResult<()> {
    close_all_sockets(&state).await;
    let panels = state.store.load_panels().map_err(|e| e.to_string())?;
    for panel in &panels {
        // Best effort: a missing keychain entry must not block disconnecting.
        let _ = secrets::delete_api_key(&panel.id);
    }
    state.store.save_panels(&[]).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> CmdResult<Vec<Server>> {
    let client = client_for(&state)?;
    client.list_servers().await.map_err(|e| e.to_string())
}

/// One-shot resource snapshot; the dashboard uses this only for the first
/// paint before the websocket delivers live data.
#[tauri::command]
pub async fn server_resources(
    state: State<'_, AppState>,
    identifier: String,
) -> CmdResult<ServerStats> {
    let client = client_for(&state)?;
    client
        .server_resources(&identifier)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_power(
    state: State<'_, AppState>,
    identifier: String,
    signal: String,
) -> CmdResult<()> {
    let signal: PowerSignal = signal.parse()?;
    let client = client_for(&state)?;
    client
        .set_power(&identifier, signal)
        .await
        .map_err(|e| e.to_string())
}

/// Open the server's websocket and forward its events to the frontend as
/// Tauri events named `server-event-{identifier}`. Idempotent.
#[tauri::command]
pub async fn subscribe_server(
    app: AppHandle,
    state: State<'_, AppState>,
    identifier: String,
) -> CmdResult<()> {
    let mut sockets = state.sockets.lock().await;
    if sockets.contains_key(&identifier) {
        return Ok(());
    }
    let client = client_for(&state)?;
    let ServerSocket {
        mut events,
        outgoing,
    } = ServerSocket::spawn(client, identifier.clone());
    let event_name = format!("server-event-{identifier}");
    let forwarder = tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            let _ = app.emit(&event_name, &event);
        }
    });
    sockets.insert(
        identifier,
        SocketHandle {
            outgoing,
            forwarder,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn unsubscribe_server(state: State<'_, AppState>, identifier: String) -> CmdResult<()> {
    if let Some(handle) = state.sockets.lock().await.remove(&identifier) {
        close_socket(handle);
    }
    Ok(())
}

#[tauri::command]
pub async fn send_console_command(
    state: State<'_, AppState>,
    identifier: String,
    command: String,
) -> CmdResult<()> {
    let sockets = state.sockets.lock().await;
    let handle = sockets
        .get(&identifier)
        .ok_or_else(|| "console is not connected".to_string())?;
    handle
        .outgoing
        .send(Outgoing::Command(command))
        .await
        .map_err(|_| "console connection closed".to_string())
}

async fn close_all_sockets(state: &AppState) {
    let mut sockets = state.sockets.lock().await;
    for (_, handle) in sockets.drain() {
        close_socket(handle);
    }
}

fn close_socket(handle: SocketHandle) {
    // Dropping the outgoing sender ends the core task; it closes the event
    // channel, which lets the forwarder finish on its own. The abort is only
    // a belt-and-braces for a forwarder stuck on a slow emit.
    drop(handle.outgoing);
    handle.forwarder.abort();
}
