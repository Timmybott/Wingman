//! The deploy engine — Feather's core feature.
//!
//! Full flow (spec 6.3): auto-commit the project (checkpoint for rollback) →
//! optional build command → optional pre-deploy backup with rotation → scan
//! honoring `.deployignore` → pack a zip → upload via the panel's signed URL
//! → decompress into the target directory → delete the remote archive →
//! delete files that were in the previous deploy but are gone locally
//! (manifest diff) → optionally restart the server.
//!
//! Rollback (spec 6.4) archives an old commit into a temp directory — the
//! working tree is never touched — and runs the same pipeline from there.
//!
//! Same supervision pattern as [`crate::ws`]: `start_deploy`/`start_rollback`
//! spawn a task and hand back an event receiver the UI renders step by step.

use crate::api::PanelClient;
use crate::config::{ConfigStore, DeployRecord, PostDeployAction, ProjectConfig};
use crate::error::Error;
use crate::git;
use crate::models::PowerSignal;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

/// Backups created by Feather carry this prefix; rotation only ever deletes
/// backups matching it — user-created backups are never touched.
pub const BACKUP_PREFIX: &str = "feather-pre-deploy-";

const BACKUP_POLL_INTERVAL: Duration = Duration::from_millis(500);
const BACKUP_POLL_ATTEMPTS: usize = 1200; // × 500 ms = 10 minutes

/// Progress events, serialized as `{"step":…, …}` for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum DeployStep {
    /// Checkpointing the project state in git before deploying.
    Committing,
    /// Rollback only: writing the old commit into a temp directory.
    CheckingOut,
    Building,
    /// One line of build command output (stdout or stderr).
    BuildOutput {
        line: String,
    },
    BackingUp,
    /// The pre-deploy backup could not be taken; the deploy continues.
    BackupSkipped {
        reason: String,
    },
    Scanning,
    Packing {
        files: usize,
    },
    Uploading {
        percent: u8,
    },
    /// Pull only (initial import / multi-device sync): fetching the server
    /// archive.
    Downloading {
        percent: u8,
    },
    /// Pull only: writing the downloaded files into the local folder.
    Importing,
    Extracting,
    CleaningUp,
    Restarting,
    Done {
        files: usize,
        deleted: usize,
    },
    Failed {
        message: String,
    },
}

pub struct DeployHandle {
    pub events: mpsc::Receiver<DeployStep>,
}

/// Run a deploy in a background task. The final event is always `Done` or
/// `Failed`; dropping the receiver mid-deploy lets the deploy finish, the
/// remaining events just go nowhere.
pub fn start_deploy(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
) -> DeployHandle {
    spawn_engine(move |tx| async move { run_deploy(&client, &store, &project, &tx).await })
}

/// Deploy an old commit (spec 6.4): the commit's tree is archived into a
/// temp directory and the normal pipeline runs from there. Uncommitted work
/// in the project folder stays untouched.
pub fn start_rollback(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
    commit_id: String,
) -> DeployHandle {
    spawn_engine(
        move |tx| async move { run_rollback(&client, &store, &project, commit_id, &tx).await },
    )
}

pub(crate) fn spawn_engine<F, Fut>(run: F) -> DeployHandle
where
    F: FnOnce(mpsc::Sender<DeployStep>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(usize, usize), Error>> + Send,
{
    let (tx, events) = mpsc::channel(256);
    let task_tx = tx.clone();
    tokio::spawn(async move {
        match run(task_tx).await {
            Ok((files, deleted)) => {
                let _ = tx.send(DeployStep::Done { files, deleted }).await;
            }
            Err(err) => {
                let _ = tx
                    .send(DeployStep::Failed {
                        message: err.to_string(),
                    })
                    .await;
            }
        }
    });
    DeployHandle { events }
}

async fn run_deploy(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let local = project.local_path.clone();
    if !local.is_dir() {
        return Err(Error::Deploy(format!(
            "project folder does not exist: {}",
            local.display()
        )));
    }

    // Checkpoint: make sure a repo exists and the deployed state is a commit.
    let _ = tx.send(DeployStep::Committing).await;
    let commit = {
        let local = local.clone();
        tokio::task::spawn_blocking(move || -> Result<Option<String>, Error> {
            git::ensure_repo(&local)?;
            let status = git::status(&local)?;
            if status.dirty {
                let message = format!("Deploy at {}", format_utc(now_secs()));
                Ok(Some(git::commit_all(&local, &message)?.id))
            } else {
                Ok(status.head.map(|head| head.id))
            }
        })
        .await
        .map_err(|e| Error::Deploy(format!("git task failed: {e}")))??
    };

    run_pipeline(client, store, project, &local, commit, tx).await
}

/// Roll the server back to a stored full snapshot (a released deploy, `kind` =
/// "rollback"): download it from the storage backend, extract it into a temp
/// directory and deploy from there. The local folder is never touched, so any
/// teammate can restore a past deploy even without those files locally.
#[allow(clippy::too_many_arguments)]
pub fn start_snapshot_rollback(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    kind: String,
    snapshot_id: String,
) -> DeployHandle {
    spawn_engine(move |tx| async move {
        run_snapshot_rollback(
            &client,
            &store,
            &project,
            &endpoint,
            &token,
            &anon_key,
            &project_id,
            &kind,
            &snapshot_id,
            &tx,
        )
        .await
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_snapshot_rollback(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    kind: &str,
    snapshot_id: &str,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let _ = tx.send(DeployStep::Downloading { percent: 0 }).await;
    let bytes = crate::snapshot::download_snapshot(
        endpoint,
        token,
        anon_key,
        project_id,
        snapshot_id,
        kind,
    )
    .await?;

    let _ = tx.send(DeployStep::CheckingOut).await;
    let temp = tempfile::tempdir()?;
    {
        let dest = temp.path().to_path_buf();
        tokio::task::spawn_blocking(move || crate::snapshot::extract_zip(&bytes, &dest))
            .await
            .map_err(|e| Error::Deploy(format!("extract task failed: {e}")))??;
    }

    // `temp` stays alive until the pipeline is done with its contents.
    run_pipeline(
        client,
        store,
        project,
        temp.path(),
        Some(snapshot_id.to_string()),
        tx,
    )
    .await
}

async fn run_rollback(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    commit_id: String,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    if !project.local_path.is_dir() {
        return Err(Error::Deploy(format!(
            "project folder does not exist: {}",
            project.local_path.display()
        )));
    }

    let _ = tx.send(DeployStep::CheckingOut).await;
    let temp = tempfile::tempdir()?;
    {
        let local = project.local_path.clone();
        let dest = temp.path().to_path_buf();
        let commit = commit_id.clone();
        tokio::task::spawn_blocking(move || git::archive_commit(&local, &commit, &dest))
            .await
            .map_err(|e| Error::Deploy(format!("archive task failed: {e}")))??;
    }

    // `temp` stays alive until the pipeline is done with its contents.
    run_pipeline(client, store, project, temp.path(), Some(commit_id), tx).await
}

/// Everything after the source is settled: build → backup → scan → pack →
/// upload → extract → cleanup → manifest diff → record → restart.
async fn run_pipeline(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    source: &Path,
    commit: Option<String>,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let root = normalize_target_dir(&project.target_dir)?;

    if let Some(command) = project
        .build_command
        .as_deref()
        .filter(|c| !c.trim().is_empty())
    {
        let _ = tx.send(DeployStep::Building).await;
        run_build(source, command, tx).await?;
    }

    if project.auto_backup {
        let _ = tx.send(DeployStep::BackingUp).await;
        ensure_backup(client, &project.server_identifier, tx).await?;
    }

    let _ = tx.send(DeployStep::Scanning).await;
    // Scanning and zipping are blocking filesystem work.
    let (manifest, archive) = {
        let source = source.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<_, Error> {
            let manifest = scan_project(&source)?;
            if manifest.is_empty() {
                return Err(Error::Deploy(
                    "nothing to deploy — the project folder is empty or everything is ignored"
                        .into(),
                ));
            }
            let archive = pack_zip(&source, &manifest)?;
            Ok((manifest, archive))
        })
        .await
        .map_err(|e| Error::Deploy(format!("packing task failed: {e}")))??
    };
    let _ = tx
        .send(DeployStep::Packing {
            files: manifest.len(),
        })
        .await;

    push_archive(client, project, &root, archive.path(), tx).await?;

    // Manifest diff: what the last deploy contained but this one doesn't
    // gets deleted remotely, so stale plugins/scripts can't linger.
    let previous = store.load_deploy_record(&project.id)?;
    let current: BTreeSet<&str> = manifest.iter().map(String::as_str).collect();
    let stale: Vec<String> = previous
        .map(|record| {
            record
                .manifest
                .into_iter()
                .filter(|path| !current.contains(path.as_str()))
                .collect()
        })
        .unwrap_or_default();
    if !stale.is_empty() {
        client
            .delete_files(&project.server_identifier, &root, &stale)
            .await?;
    }

    let deployed_at = now_secs();
    // Content manifest (path → hash) of what we just shipped, so other devices
    // can tell exactly which files this deploy changed when they sync.
    let content = {
        let source = source.to_path_buf();
        tokio::task::spawn_blocking(move || crate::snapshot::manifest_of(&source))
            .await
            .map_err(|e| Error::Deploy(format!("manifest task failed: {e}")))??
    };
    store.save_deploy_record(
        &project.id,
        &DeployRecord {
            timestamp: deployed_at,
            manifest: manifest.clone(),
            commit: commit.clone(),
            content: content.clone(),
        },
    )?;

    // Announce this deploy to other devices (multi-device sync). Best
    // effort — a panel without files/write must not fail the deploy.
    let _ = client
        .write_file(
            &project.server_identifier,
            &crate::sync::state_path(&root),
            crate::sync::state_json(deployed_at, &commit, &manifest, &content),
        )
        .await;

    if project.post_deploy == PostDeployAction::Restart {
        let _ = tx.send(DeployStep::Restarting).await;
        client
            .set_power(&project.server_identifier, PowerSignal::Restart)
            .await?;
    }

    Ok((manifest.len(), stale.len()))
}

/// Upload a packed archive into `root` on the server, decompress it and remove
/// the uploaded archive. Shared by the folder pipeline and the bundle deploy.
async fn push_archive(
    client: &PanelClient,
    project: &ProjectConfig,
    root: &str,
    archive_path: &Path,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(), Error> {
    // Make sure the target directory exists (best effort — a level that
    // already exists makes the panel answer with an error we can ignore).
    if root != "/" {
        let mut parent = String::from("/");
        for segment in root.trim_start_matches('/').split('/') {
            let _ = client
                .create_folder(&project.server_identifier, &parent, segment)
                .await;
            if parent != "/" {
                parent.push('/');
            }
            parent.push_str(segment);
        }
    }

    let remote_name = format!(".feather-deploy-{}.zip", now_secs());

    let _ = tx.send(DeployStep::Uploading { percent: 0 }).await;
    let signed_url = client.upload_url(&project.server_identifier).await?;
    let progress_tx = tx.clone();
    let mut last_percent = 0u8;
    client
        .upload_zip(
            &signed_url,
            root,
            archive_path,
            &remote_name,
            move |sent, total| {
                let percent = if total == 0 {
                    100
                } else {
                    ((sent as f64 / total as f64) * 100.0) as u8
                };
                if percent != last_percent {
                    last_percent = percent;
                    // try_send: progress must never block the upload.
                    let _ = progress_tx.try_send(DeployStep::Uploading { percent });
                }
            },
        )
        .await?;

    let _ = tx.send(DeployStep::Extracting).await;
    client
        .decompress_file(&project.server_identifier, root, &remote_name)
        .await?;

    let _ = tx.send(DeployStep::CleaningUp).await;
    client
        .delete_files(
            &project.server_identifier,
            root,
            std::slice::from_ref(&remote_name),
        )
        .await?;
    Ok(())
}

/// One commit of the current Deploy bundle, as needed to apply it: the storage
/// id of its delta snapshot and the full manifest of the tree *after* it. The
/// caller passes these oldest-first.
#[derive(Debug, Clone)]
pub struct BundleCommit {
    /// Storage commit id — used to download the commit's delta zip.
    pub id: String,
    /// Full path → hash manifest of the committed tree after this commit.
    pub manifest: crate::snapshot::Manifest,
}

/// Storage context for snapshotting the deployed state as a rollback point.
struct SnapshotCtx {
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    /// The bundle being deployed — the rollback snapshot is keyed by its id.
    bundle_id: String,
}

/// Deploy the current bundle: apply the accumulated commit deltas to the server
/// and nothing else. A deploy introduces no changes of its own — uncommitted
/// local edits are never shipped, and a member without a local folder can
/// deploy just the same. `bundle_id` keys the full-tree rollback snapshot taken
/// once the new state is live.
#[allow(clippy::too_many_arguments)]
pub fn start_bundle_deploy(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
    endpoint: String,
    token: String,
    anon_key: String,
    project_id: String,
    bundle_id: String,
    base: crate::snapshot::Manifest,
    commits: Vec<BundleCommit>,
) -> DeployHandle {
    spawn_engine(move |tx| async move {
        run_bundle_deploy(
            &client,
            &store,
            &project,
            &endpoint,
            &token,
            &anon_key,
            &project_id,
            &bundle_id,
            base,
            commits,
            &tx,
        )
        .await
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_bundle_deploy(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    bundle_id: &str,
    base: crate::snapshot::Manifest,
    commits: Vec<BundleCommit>,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    if commits.is_empty() {
        return Err(Error::Deploy(
            "nothing to deploy — commit your changes first".into(),
        ));
    }

    // Download each commit's delta zip and derive the paths it deleted from the
    // difference between its manifest and the previous one (base for the first).
    let _ = tx.send(DeployStep::Downloading { percent: 0 }).await;
    let mut deltas = Vec::with_capacity(commits.len());
    let mut prev: &crate::snapshot::Manifest = &base;
    for bc in &commits {
        let bytes = crate::snapshot::download_snapshot(
            endpoint, token, anon_key, project_id, &bc.id, "commit",
        )
        .await?;
        let deleted: Vec<String> = prev
            .keys()
            .filter(|p| !bc.manifest.contains_key(*p))
            .cloned()
            .collect();
        deltas.push(crate::snapshot::CommitDelta {
            zip: bytes,
            deleted,
        });
        prev = &bc.manifest;
    }

    let commit_id = commits.last().map(|c| c.id.clone());
    let snapshot = SnapshotCtx {
        endpoint: endpoint.to_string(),
        token: token.to_string(),
        anon_key: anon_key.to_string(),
        project_id: project_id.to_string(),
        bundle_id: bundle_id.to_string(),
    };
    apply_bundle(
        client,
        store,
        project,
        base,
        deltas,
        commit_id,
        Some(&snapshot),
        tx,
    )
    .await
}

/// Apply already-fetched commit deltas to the server: overlay them onto the
/// server baseline, upload the net changed files and delete the net removed
/// ones — nothing else. Split out from the download so it can be driven
/// directly (tests, or callers that already hold the deltas).
pub fn start_apply_bundle(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
    base: crate::snapshot::Manifest,
    deltas: Vec<crate::snapshot::CommitDelta>,
    commit_id: Option<String>,
) -> DeployHandle {
    spawn_engine(move |tx| async move {
        apply_bundle(
            &client, &store, &project, base, deltas, commit_id, None, &tx,
        )
        .await
    })
}

#[allow(clippy::too_many_arguments)]
async fn apply_bundle(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    base: crate::snapshot::Manifest,
    deltas: Vec<crate::snapshot::CommitDelta>,
    commit_id: Option<String>,
    snapshot: Option<&SnapshotCtx>,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let root = normalize_target_dir(&project.target_dir)?;

    if project.auto_backup {
        let _ = tx.send(DeployStep::BackingUp).await;
        ensure_backup(client, &project.server_identifier, tx).await?;
    }

    // Overlay the deltas into a temp tree: it ends up holding exactly the net
    // added/modified files, and reports the paths to delete on the server.
    let _ = tx.send(DeployStep::Extracting).await;
    let temp = tempfile::tempdir()?;
    let (net_deleted, resulting) = {
        let base = base.clone();
        let dest = temp.path().to_path_buf();
        tokio::task::spawn_blocking(move || {
            crate::snapshot::materialize_deltas(&base, &deltas, &dest)
        })
        .await
        .map_err(|e| Error::Deploy(format!("materialize task failed: {e}")))??
    };

    let to_upload = scan_project(temp.path())?;
    if to_upload.is_empty() && net_deleted.is_empty() {
        return Err(Error::Deploy(
            "nothing to deploy — the committed changes are already on the server".into(),
        ));
    }

    // Upload the changed files (if any).
    if !to_upload.is_empty() {
        let _ = tx
            .send(DeployStep::Packing {
                files: to_upload.len(),
            })
            .await;
        let archive = {
            let source = temp.path().to_path_buf();
            let files = to_upload.clone();
            tokio::task::spawn_blocking(move || pack_zip(&source, &files))
                .await
                .map_err(|e| Error::Deploy(format!("packing task failed: {e}")))??
        };
        push_archive(client, project, &root, archive.path(), tx).await?;
    }

    // Apply the net deletions on the server.
    if !net_deleted.is_empty() {
        let _ = tx.send(DeployStep::CleaningUp).await;
        client
            .delete_files(&project.server_identifier, &root, &net_deleted)
            .await?;
    }

    // Snapshot the full deployed tree as a rollback point (best effort). The
    // server now holds the new state, so download it and store it keyed by the
    // bundle id — rollback later restores this deploy from it. A failure here
    // (e.g. a very large tree) must never fail the deploy itself.
    if let Some(ctx) = snapshot {
        if let Ok(temp) = tempfile::tempdir() {
            match crate::sync::download_server_tree(client, project, temp.path()).await {
                Ok(_) => {
                    if let Err(e) = crate::snapshot::upload_snapshot(
                        temp.path(),
                        &ctx.endpoint,
                        &ctx.token,
                        &ctx.anon_key,
                        &ctx.project_id,
                        &ctx.bundle_id,
                        "rollback",
                    )
                    .await
                    {
                        eprintln!("rollback snapshot skipped: {e}");
                    }
                }
                Err(e) => eprintln!("rollback snapshot skipped: {e}"),
            }
        }
    }

    // Record the new full server state so future diffs/baselines are correct
    // and other devices can sync to it. The deploy record and sync marker track
    // the list of paths now on the server.
    let deployed_at = now_secs();
    let resulting_paths: Vec<String> = resulting.keys().cloned().collect();
    store.save_deploy_record(
        &project.id,
        &DeployRecord {
            timestamp: deployed_at,
            manifest: resulting_paths.clone(),
            commit: commit_id.clone(),
            content: resulting.clone(),
        },
    )?;
    let _ = client
        .write_file(
            &project.server_identifier,
            &crate::sync::state_path(&root),
            crate::sync::state_json(deployed_at, &commit_id, &resulting_paths, &resulting),
        )
        .await;

    if project.post_deploy == PostDeployAction::Restart {
        let _ = tx.send(DeployStep::Restarting).await;
        client
            .set_power(&project.server_identifier, PowerSignal::Restart)
            .await?;
    }

    Ok((resulting.len(), net_deleted.len()))
}

/// Run the configured build command through the platform shell, streaming
/// stdout/stderr lines as events. A non-zero exit aborts the deploy.
async fn run_build(dir: &Path, command: &str, tx: &mpsc::Sender<DeployStep>) -> Result<(), Error> {
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    let mut child = cmd
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Deploy(format!("failed to start build command: {e}")))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let out_task = tokio::spawn(forward_lines(stdout, tx.clone()));
    let err_task = tokio::spawn(forward_lines(stderr, tx.clone()));

    let status = child
        .wait()
        .await
        .map_err(|e| Error::Deploy(format!("build command failed to run: {e}")))?;
    let _ = out_task.await;
    let _ = err_task.await;

    if !status.success() {
        return Err(Error::Deploy(format!("build command failed ({status})")));
    }
    Ok(())
}

async fn forward_lines<R: AsyncRead + Unpin>(reader: R, tx: mpsc::Sender<DeployStep>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if tx.send(DeployStep::BuildOutput { line }).await.is_err() {
            return;
        }
    }
}

/// Take a pre-deploy backup, rotating Feather's own backups when the server's
/// backup limit is reached. Foreign backups are never deleted — in that case
/// (or with a limit of 0) the step is skipped with a note and the deploy
/// continues: a missing backup should not block shipping.
async fn ensure_backup(
    client: &PanelClient,
    identifier: &str,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(), Error> {
    let server = client.server_details(identifier).await?;
    let limit = server.feature_limits.backups.unwrap_or(0);
    if limit <= 0 {
        let _ = tx
            .send(DeployStep::BackupSkipped {
                reason: "the server has no backup slots".into(),
            })
            .await;
        return Ok(());
    }

    let backups = client.list_backups(identifier).await?;
    // Da 'limit' jetzt wieder ein i64 ist, funktioniert dieser Vergleich wieder:
    if backups.len() as i64 >= limit {
        let oldest_own = backups
            .iter()
            .filter(|b| b.name.starts_with(BACKUP_PREFIX))
            .min_by(|a, b| a.created_at.cmp(&b.created_at));
        match oldest_own {
            Some(backup) => client.delete_backup(identifier, &backup.uuid).await?,
            None => {
                let _ = tx
                    .send(DeployStep::BackupSkipped {
                        reason: format!(
                            "backup limit ({limit}) reached and none of the backups were \
                             created by Feather — not touching foreign backups"
                        ),
                    })
                    .await;
                return Ok(());
            }
        }
    }

    let name = format!("{BACKUP_PREFIX}{}", now_secs());
    let backup = client.create_backup(identifier, &name).await?;
    for _ in 0..BACKUP_POLL_ATTEMPTS {
        let details = client.backup_details(identifier, &backup.uuid).await?;
        if details.completed_at.is_some() {
            if details.is_successful {
                return Ok(());
            }
            return Err(Error::Deploy(
                "the pre-deploy backup failed on the server".into(),
            ));
        }
        tokio::time::sleep(BACKUP_POLL_INTERVAL).await;
    }
    Err(Error::Deploy(
        "timed out waiting for the pre-deploy backup to finish".into(),
    ))
}

/// All files to deploy, as sorted relative paths with forward slashes.
///
/// `.deployignore` (gitignore syntax, per directory level) controls the
/// exclusions; `.git/` and `.deployignore` itself are always excluded.
pub fn scan_project(root: &Path) -> Result<Vec<String>, Error> {
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .standard_filters(false)
        .hidden(false)
        .add_custom_ignore_filename(".deployignore");
    builder.filter_entry(|entry| entry.file_name() != std::ffi::OsStr::new(".git"));

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(|e| Error::Deploy(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("walker yields paths under root");
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if rel == ".deployignore" || rel.ends_with("/.deployignore") {
            continue;
        }
        files.push(rel);
    }
    files.sort();
    Ok(files)
}

fn pack_zip(root: &Path, manifest: &[String]) -> Result<tempfile::NamedTempFile, Error> {
    let tmp = tempfile::NamedTempFile::new()?;
    let file = tmp.reopen()?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for rel in manifest {
        zip.start_file(rel.clone(), options)
            .map_err(|e| Error::Deploy(format!("zip {rel}: {e}")))?;
        let mut src = std::fs::File::open(local_file(root, rel))?;
        std::io::copy(&mut src, &mut zip)?;
    }
    zip.finish()
        .map_err(|e| Error::Deploy(format!("finish zip: {e}")))?;
    Ok(tmp)
}

fn local_file(root: &Path, rel: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in rel.split('/') {
        path.push(segment);
    }
    path
}

/// `""`/`"/"` → `/` (server root); `"app/sub"` → `/app/sub`.
/// Rejects `..` so a target can never escape the server root.
pub fn normalize_target_dir(target: &str) -> Result<String, Error> {
    let cleaned = target.trim().replace('\\', "/");
    let segments: Vec<&str> = cleaned
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if segments.contains(&"..") {
        return Err(Error::Deploy(
            "target directory must not contain `..`".into(),
        ));
    }
    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_secs()
}

/// `2026-07-19 04:32 UTC` from unix seconds (no chrono dependency needed
/// for a commit message timestamp).
fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    let rem = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

/// Howard Hinnant's `civil_from_days` — days since 1970-01-01 → (y, m, d).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_target_dirs() {
        assert_eq!(normalize_target_dir("").unwrap(), "/");
        assert_eq!(normalize_target_dir(" / ").unwrap(), "/");
        assert_eq!(normalize_target_dir("app").unwrap(), "/app");
        assert_eq!(normalize_target_dir("/app/sub/").unwrap(), "/app/sub");
        assert_eq!(normalize_target_dir("app\\sub").unwrap(), "/app/sub");
        assert!(normalize_target_dir("../escape").is_err());
        assert!(normalize_target_dir("a/../../b").is_err());
    }

    #[test]
    fn scan_honors_deployignore_and_skips_git() {
        let dir = tempfile::tempdir().unwrap();
        let write = |rel: &str, content: &str| {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        };
        write("index.js", "console.log('hi')");
        write("config/settings.yml", "a: 1");
        write("node_modules/lib/x.js", "x");
        write(".git/HEAD", "ref: refs/heads/main");
        write(".env", "SECRET=1");
        write(".deployignore", "node_modules/\n.env\n");

        let files = scan_project(dir.path()).unwrap();
        assert_eq!(files, vec!["config/settings.yml", "index.js"]);
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(format_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_utc(1_784_468_520), "2026-07-19 13:42 UTC");
    }
}
