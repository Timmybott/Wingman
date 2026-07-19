//! IPC commands exposed to the frontend. Errors cross the IPC boundary as
//! strings, so every command maps core errors with `to_string()`.

use crate::secrets;
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::mpsc;
use wingman_core::deploy::{start_deploy, start_rollback, DeployStep};
use wingman_core::git;
use wingman_core::models::{FileEntry, PowerSignal, Server, ServerStats};
use wingman_core::sync::{is_newer, read_remote_state, start_pull, PullMode};
use wingman_core::ws::Outgoing;
use wingman_core::{
    normalize_base_url, CommitInfo, DeployHandle, PanelClient, PanelConfig, PostDeployAction,
    ProjectConfig, RepoStatus, ServerSocket,
};

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
    let api_key = secrets::get_api_key(state.store.dir(), &panel.id)?;
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
    secrets::set_api_key(state.store.dir(), &panel.id, api_key.trim())?;
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
        let _ = secrets::delete_api_key(state.store.dir(), &panel.id);
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

// ---------------------------------------------------------------------------
// Projects & deploy
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> CmdResult<Vec<ProjectConfig>> {
    state.store.load_projects().map_err(|e| e.to_string())
}

/// Create or update a project. An empty id means "new". One project per
/// server in v1 — a second link to the same server is rejected. Linking
/// also makes sure the folder is a git repository (spec: the app
/// initializes one when none exists).
#[tauri::command]
pub async fn save_project(
    state: State<'_, AppState>,
    mut project: ProjectConfig,
) -> CmdResult<ProjectConfig> {
    if !project.local_path.is_dir() {
        return Err(format!(
            "project folder does not exist: {}",
            project.local_path.display()
        ));
    }
    {
        let path = project.local_path.clone();
        tokio::task::spawn_blocking(move || git::ensure_repo(&path))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    }
    if project.id.is_empty() {
        project.id = wingman_core::config::new_project_id();
    }
    if project.name.trim().is_empty() {
        project.name = project
            .local_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Project".into());
    }

    let mut projects = state.store.load_projects().map_err(|e| e.to_string())?;
    let clash = projects
        .iter()
        .any(|p| p.server_identifier == project.server_identifier && p.id != project.id);
    if clash {
        return Err("this server already has a linked project".into());
    }
    match projects.iter_mut().find(|p| p.id == project.id) {
        Some(existing) => *existing = project.clone(),
        None => projects.push(project.clone()),
    }
    state
        .store
        .save_projects(&projects)
        .map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, project_id: String) -> CmdResult<()> {
    let mut projects = state.store.load_projects().map_err(|e| e.to_string())?;
    projects.retain(|p| p.id != project_id);
    state
        .store
        .save_projects(&projects)
        .map_err(|e| e.to_string())?;
    let _ = state.store.delete_deploy_record(&project_id);
    Ok(())
}

fn find_project(state: &AppState, project_id: &str) -> CmdResult<ProjectConfig> {
    state
        .store
        .load_projects()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| "project not found".to_string())
}

/// Kick off a deploy. Progress is emitted as `deploy-event-{project_id}`
/// Tauri events; a second deploy of the same project while one is running
/// is rejected. Desktop notifications fire on failure, and on success when
/// the project's post-deploy behavior is "notify".
#[tauri::command]
pub async fn deploy_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> CmdResult<()> {
    let project = find_project(&state, &project_id)?;
    let client = client_for(&state)?;
    claim_engine_slot(&state, &project_id).await?;
    let handle = start_deploy(client, state.store.clone(), project.clone());
    forward_engine_events(app, project, project_id, handle, "Deploy");
    Ok(())
}

/// Deploy an old commit (spec 6.4). Shares the running-guard and event
/// channel with deploy_project.
#[tauri::command]
pub async fn rollback_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    commit_id: String,
) -> CmdResult<()> {
    let project = find_project(&state, &project_id)?;
    let client = client_for(&state)?;
    claim_engine_slot(&state, &project_id).await?;
    let handle = start_rollback(client, state.store.clone(), project.clone(), commit_id);
    forward_engine_events(app, project, project_id, handle, "Rollback");
    Ok(())
}

/// Pull the server state into the local folder. `mode` is "import" (only
/// into an empty folder, right after linking) or "sync" (only with a clean
/// working tree — multi-device sync). Progress shares the deploy-event
/// channel.
#[tauri::command]
pub async fn pull_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    mode: String,
) -> CmdResult<()> {
    let mode = match mode.as_str() {
        "import" => PullMode::InitialImport,
        "sync" => PullMode::SyncIfClean,
        other => return Err(format!("unknown pull mode `{other}`")),
    };
    let project = find_project(&state, &project_id)?;
    let client = client_for(&state)?;
    claim_engine_slot(&state, &project_id).await?;
    let handle = start_pull(client, state.store.clone(), project.clone(), mode);
    forward_engine_events(app, project, project_id, handle, "Sync");
    Ok(())
}

#[derive(serde::Serialize)]
pub struct RemoteDeployInfo {
    /// A different deploy than this device's record exists on the server.
    pub newer: bool,
    /// Local uncommitted changes — auto-sync must not run.
    pub dirty: bool,
}

/// Poll target for multi-device sync: does the server announce a deploy
/// this device hasn't picked up yet?
#[tauri::command]
pub async fn check_remote_deploy(
    state: State<'_, AppState>,
    project_id: String,
) -> CmdResult<RemoteDeployInfo> {
    let project = find_project(&state, &project_id)?;
    let client = client_for(&state)?;
    let Some(remote) = read_remote_state(&client, &project)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(RemoteDeployInfo {
            newer: false,
            dirty: false,
        });
    };
    let record = state
        .store
        .load_deploy_record(&project_id)
        .map_err(|e| e.to_string())?;
    let newer = is_newer(&remote, record.as_ref());
    let dirty = if newer {
        let path = project.local_path.clone();
        tokio::task::spawn_blocking(move || -> Result<bool, wingman_core::Error> {
            git::ensure_repo(&path)?;
            Ok(git::status(&path)?.dirty)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    } else {
        false
    };
    Ok(RemoteDeployInfo { newer, dirty })
}

async fn claim_engine_slot(state: &AppState, project_id: &str) -> CmdResult<()> {
    let mut running = state.deploys.lock().await;
    if !running.insert(project_id.to_string()) {
        return Err("a deploy for this project is already running".into());
    }
    Ok(())
}

fn forward_engine_events(
    app: AppHandle,
    project: ProjectConfig,
    project_id: String,
    mut handle: DeployHandle,
    verb: &'static str,
) {
    let event_name = format!("deploy-event-{project_id}");
    tauri::async_runtime::spawn(async move {
        while let Some(step) = handle.events.recv().await {
            match &step {
                DeployStep::Done { files, .. } => {
                    if project.post_deploy == PostDeployAction::Notify {
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("{verb} finished — {}", project.name))
                            .body(format!("{files} files deployed. Server was not restarted."))
                            .show();
                    }
                }
                DeployStep::Failed { message } => {
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("{verb} failed — {}", project.name))
                        .body(message.clone())
                        .show();
                }
                _ => {}
            }
            let _ = app.emit(&event_name, &step);
        }
        let state: State<'_, AppState> = app.state();
        state.deploys.lock().await.remove(&project_id);
    });
}

// ---------------------------------------------------------------------------
// Server file browser
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_server_files(
    state: State<'_, AppState>,
    identifier: String,
    directory: String,
) -> CmdResult<Vec<FileEntry>> {
    let client = client_for(&state)?;
    client
        .list_files(&identifier, &directory)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_server_files(
    state: State<'_, AppState>,
    identifier: String,
    root: String,
    files: Vec<String>,
) -> CmdResult<()> {
    let client = client_for(&state)?;
    client
        .delete_files(&identifier, &root, &files)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_server_folder(
    state: State<'_, AppState>,
    identifier: String,
    root: String,
    name: String,
) -> CmdResult<()> {
    let client = client_for(&state)?;
    client
        .create_folder(&identifier, &root, &name)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Git: status, commits, history
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn repo_status(state: State<'_, AppState>, project_id: String) -> CmdResult<RepoStatus> {
    let project = find_project(&state, &project_id)?;
    tokio::task::spawn_blocking(move || {
        git::ensure_repo(&project.local_path)?;
        git::status(&project.local_path)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn commit_project(
    state: State<'_, AppState>,
    project_id: String,
    message: String,
) -> CmdResult<CommitInfo> {
    let project = find_project(&state, &project_id)?;
    let message = match message.trim() {
        "" => "Checkpoint".to_string(),
        trimmed => trimmed.to_string(),
    };
    tokio::task::spawn_blocking(move || {
        git::ensure_repo(&project.local_path)?;
        git::commit_all(&project.local_path, &message)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_history(
    state: State<'_, AppState>,
    project_id: String,
    limit: Option<usize>,
) -> CmdResult<Vec<CommitInfo>> {
    let project = find_project(&state, &project_id)?;
    tokio::task::spawn_blocking(move || {
        git::ensure_repo(&project.local_path)?;
        git::log(&project.local_path, limit.unwrap_or(50))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct LastDeployInfo {
    pub timestamp: u64,
    pub commit: Option<String>,
}

#[derive(serde::Serialize)]
pub struct DeployStatus {
    pub last_deploy: Option<LastDeployInfo>,
    /// Commits on top of the deployed one; `None` when nothing was deployed
    /// yet or the deployed commit is unknown to the repo.
    pub commits_since: Option<usize>,
}

#[tauri::command]
pub async fn deploy_status(
    state: State<'_, AppState>,
    project_id: String,
) -> CmdResult<DeployStatus> {
    let project = find_project(&state, &project_id)?;
    let record = state
        .store
        .load_deploy_record(&project_id)
        .map_err(|e| e.to_string())?;
    let Some(record) = record else {
        return Ok(DeployStatus {
            last_deploy: None,
            commits_since: None,
        });
    };
    let commits_since = match record.commit.clone() {
        Some(commit) => {
            let path = project.local_path.clone();
            tokio::task::spawn_blocking(move || git::commits_ahead(&path, &commit))
                .await
                .ok()
                .and_then(Result::ok)
        }
        None => None,
    };
    Ok(DeployStatus {
        last_deploy: Some(LastDeployInfo {
            timestamp: record.timestamp,
            commit: record.commit,
        }),
        commits_since,
    })
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
