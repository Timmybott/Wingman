//! IPC commands exposed to the frontend. Errors cross the IPC boundary as
//! strings, so every command maps core errors with `to_string()`.

use crate::secrets;
use crate::AppState;
use tauri::State;
use wingman_core::models::{Server, ServerStats};
use wingman_core::{normalize_base_url, PanelClient, PanelConfig};

type CmdResult<T> = Result<T, String>;

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

/// Remove the panel connection and its keychain entry.
#[tauri::command]
pub async fn remove_panel(state: State<'_, AppState>) -> CmdResult<()> {
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
