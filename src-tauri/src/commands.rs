//! IPC commands exposed to the frontend. Errors cross the IPC boundary as
//! strings, so every command maps core errors with `to_string()`.

use crate::AppState;
use feather_core::deploy::{
    start_bundle_deploy, start_deploy, start_rollback, start_snapshot_rollback, BundleCommit,
    DeployStep,
};
use feather_core::git;
use feather_core::models::{FileEntry, PowerSignal, Server, ServerStats};
use feather_core::snapshot;
use feather_core::sync::{is_newer, read_remote_state, start_pull, PullMode};
use feather_core::ws::Outgoing;
use feather_core::{
    normalize_base_url, CommitInfo, DeployHandle, PanelClient, PostDeployAction, ProjectConfig,
    RepoStatus, ServerSocket,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::mpsc;

type CmdResult<T> = Result<T, String>;

/// A subscribed server: sender half of the websocket task plus the forwarder
/// that turns core events into Tauri events for the frontend.
pub struct SocketHandle {
    outgoing: mpsc::Sender<Outgoing>,
    forwarder: tauri::async_runtime::JoinHandle<()>,
}

/// A Pterodactyl panel connected this session. Its credentials live encrypted
/// in the cloud (Supabase) and are decrypted per team member; this device holds
/// them in memory only while the panel is connected — never on local disk.
#[derive(Clone)]
pub struct ActivePanel {
    pub base_url: String,
    pub api_key: String,
}

/// Build a client for a specific connected panel (by cloud panel id).
fn client_for(state: &AppState, panel_id: &str) -> CmdResult<PanelClient> {
    let panel = state
        .panels
        .lock()
        .expect("panels mutex poisoned")
        .get(panel_id)
        .cloned()
        .ok_or_else(|| "panel not connected".to_string())?;
    PanelClient::new(&panel.base_url, &panel.api_key).map_err(|e| e.to_string())
}

/// Dry-run credentials check; returns the number of visible servers.
#[tauri::command]
pub async fn test_connection(base_url: String, api_key: String) -> CmdResult<usize> {
    let client = PanelClient::new(&base_url, &api_key).map_err(|e| e.to_string())?;
    let servers = client.list_servers().await.map_err(|e| e.to_string())?;
    Ok(servers.len())
}

/// Connect a team panel for this session by loading its decrypted credentials
/// into memory, keyed by its cloud panel id. The frontend fetches the key from
/// the cloud (panel_api_key RPC, team-members only) and hands it here; it is
/// never persisted locally. Several panels can be connected at once.
#[tauri::command]
pub async fn set_active_panel(
    state: State<'_, AppState>,
    panel_id: String,
    base_url: String,
    api_key: String,
) -> CmdResult<()> {
    // Reconnecting the same panel replaces its creds; drop its old sockets.
    close_panel_sockets(&state, &panel_id).await;
    let url = normalize_base_url(&base_url).map_err(|e| e.to_string())?;
    state.panels.lock().expect("panels mutex poisoned").insert(
        panel_id,
        ActivePanel {
            base_url: url.to_string(),
            api_key: api_key.trim().to_string(),
        },
    );
    Ok(())
}

/// Disconnect one panel and close its live sockets. The in-memory credentials
/// are dropped; the cloud copy is untouched.
#[tauri::command]
pub async fn clear_active_panel(state: State<'_, AppState>, panel_id: String) -> CmdResult<()> {
    close_panel_sockets(&state, &panel_id).await;
    state
        .panels
        .lock()
        .expect("panels mutex poisoned")
        .remove(&panel_id);
    Ok(())
}

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>, panel_id: String) -> CmdResult<Vec<Server>> {
    let client = client_for(&state, &panel_id)?;
    client.list_servers().await.map_err(|e| e.to_string())
}

/// One-shot resource snapshot; the dashboard uses this only for the first
/// paint before the websocket delivers live data.
#[tauri::command]
pub async fn server_resources(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
) -> CmdResult<ServerStats> {
    let client = client_for(&state, &panel_id)?;
    client
        .server_resources(&identifier)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_power(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    signal: String,
) -> CmdResult<()> {
    let signal: PowerSignal = signal.parse()?;
    let client = client_for(&state, &panel_id)?;
    client
        .set_power(&identifier, signal)
        .await
        .map_err(|e| e.to_string())
}

/// Open the server's websocket and forward its events to the frontend as
/// Tauri events named `server-event-{panel_id}-{identifier}`. Idempotent.
#[tauri::command]
pub async fn subscribe_server(
    app: AppHandle,
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
) -> CmdResult<()> {
    let key = (panel_id.clone(), identifier.clone());
    let mut sockets = state.sockets.lock().await;
    if sockets.contains_key(&key) {
        return Ok(());
    }
    let client = client_for(&state, &panel_id)?;
    let ServerSocket {
        mut events,
        outgoing,
    } = ServerSocket::spawn(client, identifier.clone());
    let event_name = format!("server-event-{panel_id}-{identifier}");
    let forwarder = tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            let _ = app.emit(&event_name, &event);
        }
    });
    sockets.insert(
        key,
        SocketHandle {
            outgoing,
            forwarder,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn unsubscribe_server(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
) -> CmdResult<()> {
    if let Some(handle) = state.sockets.lock().await.remove(&(panel_id, identifier)) {
        close_socket(handle);
    }
    Ok(())
}

#[tauri::command]
pub async fn send_console_command(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    command: String,
) -> CmdResult<()> {
    let sockets = state.sockets.lock().await;
    let handle = sockets
        .get(&(panel_id, identifier))
        .ok_or_else(|| "console is not connected".to_string())?;
    handle
        .outgoing
        .send(Outgoing::Command(command))
        .await
        .map_err(|_| "console connection closed".to_string())
}

// ---------------------------------------------------------------------------
// Per-device local folder bindings for cloud projects
// ---------------------------------------------------------------------------

/// Bind a cloud project to a local folder on this device, making sure it is a
/// git repository (initializing one if needed). Returns whether the folder is
/// currently empty, so the caller can offer to import the server's files
/// before the first deploy.
#[tauri::command]
pub fn set_project_path(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> CmdResult<bool> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("folder does not exist: {path}"));
    }
    let empty = std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    git::ensure_repo(dir).map_err(|e| e.to_string())?;
    let mut map = state
        .store
        .load_project_paths()
        .map_err(|e| e.to_string())?;
    map.insert(project_id, path);
    state
        .store
        .save_project_paths(&map)
        .map_err(|e| e.to_string())?;
    Ok(empty)
}

/// The local folder bound to a project on this device, if any.
#[tauri::command]
pub fn get_project_path(
    state: State<'_, AppState>,
    project_id: String,
) -> CmdResult<Option<String>> {
    Ok(state
        .store
        .load_project_paths()
        .map_err(|e| e.to_string())?
        .get(&project_id)
        .cloned())
}

/// Remove this device's local binding for a project (does not touch files).
#[tauri::command]
pub fn remove_project_path(state: State<'_, AppState>, project_id: String) -> CmdResult<()> {
    let mut map = state
        .store
        .load_project_paths()
        .map_err(|e| e.to_string())?;
    map.remove(&project_id);
    state
        .store
        .save_project_paths(&map)
        .map_err(|e| e.to_string())
}

/// A path is deep enough that deleting it recursively can't hit a filesystem
/// root or a bare home directory (e.g. `/`, `/home`, `/home/user`). Guards the
/// "delete everywhere" tombstone action.
fn safe_to_delete(dir: &std::path::Path) -> bool {
    dir.components().count() >= 4
}

/// Remove a project from this device: drop its binding and deploy record, and
/// — when `delete_files` — recursively delete the bound folder. Used for
/// "Remove from Feather" (files kept) and for processing a "delete everywhere"
/// tombstone (files deleted). Best effort; a missing folder is not an error.
#[tauri::command]
pub fn remove_local_project(
    state: State<'_, AppState>,
    project_id: String,
    delete_files: bool,
) -> CmdResult<()> {
    let mut map = state
        .store
        .load_project_paths()
        .map_err(|e| e.to_string())?;
    if let Some(path) = map.remove(&project_id) {
        if delete_files {
            let dir = std::path::Path::new(&path);
            if dir.is_dir() && safe_to_delete(dir) {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
    state
        .store
        .save_project_paths(&map)
        .map_err(|e| e.to_string())?;
    let _ = state.store.delete_deploy_record(&project_id);
    Ok(())
}

/// Kick off a deploy. Progress is emitted as `deploy-event-{project_id}`
/// Tauri events; a second deploy of the same project while one is running
/// is rejected. Desktop notifications fire on failure, and on success when
/// the project's post-deploy behavior is "notify".
#[tauri::command]
pub async fn deploy_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project: ProjectConfig,
) -> CmdResult<()> {
    let client = client_for(&state, &project.panel_id)?;
    let project_id = project.id.clone();
    claim_engine_slot(&state, &project_id).await?;
    let handle = start_deploy(client, state.store.clone(), project.clone());
    forward_engine_events(app, project, project_id, handle, "Deploy");
    Ok(())
}

/// A commit of the current Deploy bundle, passed from the frontend oldest
/// first: its storage id and the full manifest of the committed tree after it.
#[derive(serde::Deserialize)]
pub struct BundleCommitArg {
    pub id: String,
    pub manifest: snapshot::Manifest,
}

/// Deploy the current bundle: apply its commits' deltas to the server (over the
/// `base` server state) and nothing else. Uncommitted local edits are never
/// shipped. Shares the running-guard and event channel with deploy_project.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn deploy_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    project: ProjectConfig,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    base: snapshot::Manifest,
    commits: Vec<BundleCommitArg>,
) -> CmdResult<()> {
    let client = client_for(&state, &project.panel_id)?;
    let engine_id = project.id.clone();
    claim_engine_slot(&state, &engine_id).await?;
    let commits = commits
        .into_iter()
        .map(|c| BundleCommit {
            id: c.id,
            manifest: c.manifest,
        })
        .collect();
    let handle = start_bundle_deploy(
        client,
        state.store.clone(),
        project.clone(),
        endpoint,
        token,
        anon_key,
        project_id,
        base,
        commits,
    );
    forward_engine_events(app, project, engine_id, handle, "Deploy");
    Ok(())
}

/// Deploy an old commit (spec 6.4). Shares the running-guard and event
/// channel with deploy_project.
#[tauri::command]
pub async fn rollback_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project: ProjectConfig,
    commit_id: String,
) -> CmdResult<()> {
    let client = client_for(&state, &project.panel_id)?;
    let project_id = project.id.clone();
    claim_engine_slot(&state, &project_id).await?;
    let handle = start_rollback(client, state.store.clone(), project.clone(), commit_id);
    forward_engine_events(app, project, project_id, handle, "Rollback");
    Ok(())
}

/// Roll the server back to a cloud commit: download that commit's snapshot
/// from the storage backend and deploy it. Shares the running-guard and event
/// channel with deploy_project; the local folder is not touched.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn rollback_to_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    project: ProjectConfig,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    commit_id: String,
) -> CmdResult<()> {
    let client = client_for(&state, &project.panel_id)?;
    let engine_id = project.id.clone();
    claim_engine_slot(&state, &engine_id).await?;
    let handle = start_snapshot_rollback(
        client,
        state.store.clone(),
        project.clone(),
        endpoint,
        token,
        anon_key,
        project_id,
        commit_id,
    );
    forward_engine_events(app, project, engine_id, handle, "Rollback");
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
    project: ProjectConfig,
    mode: String,
) -> CmdResult<()> {
    let mode = match mode.as_str() {
        "import" => PullMode::InitialImport,
        "sync" => PullMode::SyncIfClean,
        other => return Err(format!("unknown pull mode `{other}`")),
    };
    let client = client_for(&state, &project.panel_id)?;
    let project_id = project.id.clone();
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
    project: ProjectConfig,
) -> CmdResult<RemoteDeployInfo> {
    let client = client_for(&state, &project.panel_id)?;
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
        .load_deploy_record(&project.id)
        .map_err(|e| e.to_string())?;
    let newer = is_newer(&remote, record.as_ref());
    let dirty = if newer {
        let path = project.local_path.clone();
        tokio::task::spawn_blocking(move || -> Result<bool, feather_core::Error> {
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
                DeployStep::BackupSkipped { reason } => {
                    // A missing backup is not fatal, but the user must know it
                    // was NOT taken (e.g. all backup slots are foreign).
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("No backup taken — {}", project.name))
                        .body(reason.clone())
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
    panel_id: String,
    identifier: String,
    directory: String,
) -> CmdResult<Vec<FileEntry>> {
    let client = client_for(&state, &panel_id)?;
    client
        .list_files(&identifier, &directory)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_server_files(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    root: String,
    files: Vec<String>,
) -> CmdResult<()> {
    let client = client_for(&state, &panel_id)?;
    client
        .delete_files(&identifier, &root, &files)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_server_folder(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    root: String,
    name: String,
) -> CmdResult<()> {
    let client = client_for(&state, &panel_id)?;
    client
        .create_folder(&identifier, &root, &name)
        .await
        .map_err(|e| e.to_string())
}

/// Read a server file as UTF-8 text. Errors if the file isn't valid UTF-8 (so
/// the UI can treat it as a non-editable binary).
#[tauri::command]
pub async fn read_server_file(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    path: String,
) -> CmdResult<String> {
    let client = client_for(&state, &panel_id)?;
    let bytes = client
        .read_file(&identifier, &path)
        .await
        .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "this file is not UTF-8 text".to_string())
}

/// Overwrite a server file with new text content.
#[tauri::command]
pub async fn write_server_file(
    state: State<'_, AppState>,
    panel_id: String,
    identifier: String,
    path: String,
    content: String,
) -> CmdResult<()> {
    let client = client_for(&state, &panel_id)?;
    client
        .write_file(&identifier, &path, content.into_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Read a file inside the project's local folder as UTF-8 text. The relative
/// path is sanitized so it can never escape the project folder.
#[tauri::command]
pub async fn read_local_file(project: ProjectConfig, path: String) -> CmdResult<String> {
    let mut full = project.local_path.clone();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err("invalid path".into());
        }
        full.push(segment);
    }
    tokio::task::spawn_blocking(move || std::fs::read(&full))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| "this file is not UTF-8 text".to_string())
        })
}

// ---------------------------------------------------------------------------
// Git: status, commits, history
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn repo_status(project: ProjectConfig) -> CmdResult<RepoStatus> {
    tokio::task::spawn_blocking(move || {
        git::ensure_repo(&project.local_path)?;
        git::status(&project.local_path)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn commit_project(project: ProjectConfig, message: String) -> CmdResult<CommitInfo> {
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

/// The content manifest (path → hash) of the project's local folder.
#[tauri::command]
pub async fn project_manifest(project: ProjectConfig) -> CmdResult<snapshot::Manifest> {
    tokio::task::spawn_blocking(move || snapshot::manifest_of(&project.local_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Diff the project's local folder against a base manifest (e.g. the current
/// server state), yielding added/modified/deleted changes.
#[tauri::command]
pub async fn project_diff(
    project: ProjectConfig,
    base: snapshot::Manifest,
) -> CmdResult<snapshot::Diff> {
    tokio::task::spawn_blocking(move || snapshot::diff_against(&project.local_path, &base))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Result of packing + uploading a commit snapshot to the storage backend.
#[derive(serde::Serialize)]
pub struct SnapshotUpload {
    pub files: usize,
    pub manifest: snapshot::Manifest,
}

/// Pack the local folder into a snapshot zip and upload it to the storage
/// backend through the `feather-storage` Edge Function. The function holds the
/// storage key and derives the path from the ids; we pass the caller's session
/// token so it can authorize the write.
#[tauri::command]
pub async fn upload_commit_snapshot(
    project: ProjectConfig,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    commit_id: String,
) -> CmdResult<SnapshotUpload> {
    let (files, manifest) = snapshot::upload_snapshot(
        &project.local_path,
        &endpoint,
        &token,
        &anon_key,
        &project_id,
        &commit_id,
        "commit",
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(SnapshotUpload { files, manifest })
}

/// Pack only what changed relative to `base` (the accumulated committed state)
/// and upload it as this commit's delta. Returns the number of changed paths
/// and the full resulting manifest, which is recorded as the commit's state so
/// a deploy can apply the whole bundle.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn upload_commit_delta(
    project: ProjectConfig,
    base: snapshot::Manifest,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    commit_id: String,
) -> CmdResult<SnapshotUpload> {
    let (files, manifest) = snapshot::upload_delta(
        &project.local_path,
        &base,
        &endpoint,
        &token,
        &anon_key,
        &project_id,
        &commit_id,
        "commit",
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(SnapshotUpload { files, manifest })
}

/// One file's text from a commit's stored snapshot. `found` is false when the
/// path is not in that commit's (delta) zip — the caller can then walk back to
/// the commit that actually wrote it.
#[derive(serde::Serialize)]
pub struct SnapshotFile {
    pub found: bool,
    pub text: String,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn snapshot_file(
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    commit_id: String,
    path: String,
) -> CmdResult<SnapshotFile> {
    let text = snapshot::snapshot_file(
        &endpoint,
        &token,
        &anon_key,
        &project_id,
        &commit_id,
        "commit",
        &path,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(SnapshotFile {
        found: text.is_some(),
        text: text.unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn project_history(
    project: ProjectConfig,
    limit: Option<usize>,
) -> CmdResult<Vec<CommitInfo>> {
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
    project: ProjectConfig,
) -> CmdResult<DeployStatus> {
    let record = state
        .store
        .load_deploy_record(&project.id)
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

/// Close every live socket belonging to one panel (used when it reconnects or
/// disconnects). Sockets are keyed by (panel id, server identifier).
async fn close_panel_sockets(state: &AppState, panel_id: &str) {
    let mut sockets = state.sockets.lock().await;
    let keys: Vec<(String, String)> = sockets
        .keys()
        .filter(|(pid, _)| pid == panel_id)
        .cloned()
        .collect();
    for key in keys {
        if let Some(handle) = sockets.remove(&key) {
            close_socket(handle);
        }
    }
}

fn close_socket(handle: SocketHandle) {
    // Dropping the outgoing sender ends the core task; it closes the event
    // channel, which lets the forwarder finish on its own. The abort is only
    // a belt-and-braces for a forwarder stuck on a slow emit.
    drop(handle.outgoing);
    handle.forwarder.abort();
}
