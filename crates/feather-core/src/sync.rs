//! Server → local synchronization.
//!
//! Two use cases share one pull pipeline:
//! - **Initial import**: when a project is linked to a server whose folder is
//!   still empty, the server's current files are downloaded into it.
//! - **Multi-device sync**: every deploy writes a small state file
//!   (`.feather-state.json`) into the target directory, including a content
//!   manifest of the deployed tree. Other devices poll it; when it announces a
//!   newer deploy they pull it automatically — unless [`sync_conflict`] finds
//!   the pull would overwrite un-deployed local work on a file the deploy does
//!   not change, in which case they hold back until it's committed/deployed.
//!
//! Pull mechanics: list the target directory → server-side compress
//! (tar.gz) → signed-URL download → extract into the project folder →
//! remove the remote archive → align the local deploy record → auto-commit.

use crate::api::PanelClient;
use crate::config::{ConfigStore, DeployRecord, ProjectConfig};
use crate::deploy::{normalize_target_dir, spawn_engine, DeployHandle, DeployStep};
use crate::error::Error;
use crate::git;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use tokio::sync::mpsc;

/// Name of the deploy-state marker inside the target directory.
pub const STATE_FILE: &str = ".feather-state.json";

/// What the last Feather deploy to this server looked like — written by the
/// deploying device, read by all others.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteState {
    pub version: u32,
    /// Unix seconds of the deploy.
    pub timestamp: u64,
    pub commit: Option<String>,
    pub manifest: Vec<String>,
    /// Content manifest (path → hash) of the deployed tree. Empty on markers
    /// written before this was tracked. Powers the precise sync guard.
    #[serde(default)]
    pub content: crate::snapshot::Manifest,
}

/// How a pull decides whether it is allowed to touch the local folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    /// Only pull when the local folder is empty (apart from `.git`) —
    /// used right after linking a project.
    InitialImport,
    /// Pull the server state into the local folder unconditionally. Multi-device
    /// sync uses this *after* [`sync_conflict`] has confirmed the pull would not
    /// destroy un-deployed local work — the safety check lives at the caller so
    /// the decision can be content-aware, not just "is the tree dirty".
    Sync,
}

/// Whether auto-syncing a new deploy into the local folder would destroy
/// un-deployed local work. A sync overwrites every file the incoming deploy
/// carries, so it is unsafe exactly when the local copy of such a file differs
/// from the deploy **and** the deploy did not change that file (its version
/// equals the baseline the device last had): that difference is a local edit
/// nobody deployed, and it must not be silently clobbered. Files the deploy
/// *does* change are expected to be replaced; local-only files the deploy does
/// not carry are left untouched by the pull, so neither blocks the sync.
pub fn sync_conflict(
    baseline: &crate::snapshot::Manifest,
    remote: &crate::snapshot::Manifest,
    local: &crate::snapshot::Manifest,
) -> bool {
    // Without a known baseline (a legacy record) we can't tell a deploy change
    // from a local edit, so be conservative: any file the pull would overwrite
    // with different content counts as a conflict.
    if baseline.is_empty() {
        return remote
            .iter()
            .any(|(path, hash)| local.get(path).is_some_and(|h| h != hash));
    }
    remote
        .iter()
        .any(|(path, remote_hash)| match local.get(path) {
            Some(local_hash) if local_hash != remote_hash => {
                baseline.get(path) == Some(remote_hash)
            }
            _ => false,
        })
}

/// Read the remote state marker; `None` when it is missing or unreadable
/// (e.g. the server was never deployed to by Feather).
pub async fn read_remote_state(
    client: &PanelClient,
    project: &ProjectConfig,
) -> Result<Option<RemoteState>, Error> {
    let root = normalize_target_dir(&project.target_dir)?;
    let path = state_path(&root);
    match client.read_file(&project.server_identifier, &path).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
        Err(_) => Ok(None),
    }
}

/// Whether the remote deploy differs from what this device last recorded —
/// the trigger for multi-device sync.
pub fn is_newer(remote: &RemoteState, local: Option<&DeployRecord>) -> bool {
    match local {
        None => true,
        Some(record) => {
            remote.timestamp > record.timestamp
                || (remote.commit.is_some() && remote.commit != record.commit)
        }
    }
}

/// Serialize the state marker for a just-finished deploy.
pub fn state_json(
    timestamp: u64,
    commit: &Option<String>,
    manifest: &[String],
    content: &crate::snapshot::Manifest,
) -> Vec<u8> {
    serde_json::to_vec_pretty(&RemoteState {
        version: 1,
        timestamp,
        commit: commit.clone(),
        manifest: manifest.to_vec(),
        content: content.clone(),
    })
    .expect("state serializes")
}

pub fn state_path(root: &str) -> String {
    if root == "/" {
        format!("/{STATE_FILE}")
    } else {
        format!("{root}/{STATE_FILE}")
    }
}

/// Pull the server's target directory into the local project folder.
/// Progress arrives as [`DeployStep`] events (Downloading/Importing/…).
pub fn start_pull(
    client: PanelClient,
    store: ConfigStore,
    project: ProjectConfig,
    mode: PullMode,
) -> DeployHandle {
    spawn_engine(move |tx| async move { run_pull(&client, &store, &project, mode, &tx).await })
}

async fn run_pull(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    mode: PullMode,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let local = project.local_path.clone();
    if !local.is_dir() {
        return Err(Error::Deploy(format!(
            "project folder does not exist: {}",
            local.display()
        )));
    }
    let root = normalize_target_dir(&project.target_dir)?;

    // Guard the local folder according to the mode.
    {
        let local = local.clone();
        let allowed = tokio::task::spawn_blocking(move || -> Result<Option<String>, Error> {
            match mode {
                PullMode::InitialImport => {
                    if dir_has_content(&local)? {
                        Ok(Some(
                            "the folder already has files — not importing over them".into(),
                        ))
                    } else {
                        Ok(None)
                    }
                }
                PullMode::Sync => {
                    // The caller decides whether syncing is safe (content-aware,
                    // see `sync_conflict`); here we just make sure a repo exists
                    // so the pulled state can be checkpointed.
                    git::ensure_repo(&local)?;
                    Ok(None)
                }
            }
        })
        .await
        .map_err(|e| Error::Deploy(format!("local check failed: {e}")))??;
        if let Some(reason) = allowed {
            let _ = tx.send(DeployStep::BackupSkipped { reason }).await;
            return Ok((0, 0));
        }
    }

    let remote_state = read_remote_state(client, project).await?;

    // Everything in the target directory except our own state marker.
    let entries: Vec<String> = client
        .list_files(&project.server_identifier, &root)
        .await?
        .into_iter()
        .map(|e| e.name)
        .filter(|name| name != STATE_FILE)
        .collect();
    if entries.is_empty() {
        let _ = tx
            .send(DeployStep::BackupSkipped {
                reason: "the server has no files to import".into(),
            })
            .await;
        return Ok((0, 0));
    }

    let _ = tx.send(DeployStep::Downloading { percent: 0 }).await;
    let archive = client
        .compress_files(&project.server_identifier, &root, &entries)
        .await?;
    let archive_path = if root == "/" {
        format!("/{}", archive.name)
    } else {
        format!("{root}/{}", archive.name)
    };
    let signed = client
        .download_url(&project.server_identifier, &archive_path)
        .await?;
    let progress_tx = tx.clone();
    let mut last_percent = 0u8;
    let bytes = client
        .download_bytes(&signed, move |received, total| {
            let percent = if total == 0 {
                0
            } else {
                ((received as f64 / total as f64) * 100.0) as u8
            };
            if percent != last_percent {
                last_percent = percent;
                let _ = progress_tx.try_send(DeployStep::Downloading { percent });
            }
        })
        .await?;
    // Best effort — a leftover archive is ugly but harmless.
    let _ = client
        .delete_files(
            &project.server_identifier,
            &root,
            std::slice::from_ref(&archive.name),
        )
        .await;

    let _ = tx.send(DeployStep::Importing).await;
    let files = {
        let local = local.clone();
        tokio::task::spawn_blocking(move || extract_tar_gz(&bytes, &local))
            .await
            .map_err(|e| Error::Deploy(format!("extract task failed: {e}")))??
    };

    // Align the local deploy record with what is now on disk, so the next
    // deploy's manifest diff and the footer counter stay correct.
    let record = match &remote_state {
        Some(state) => DeployRecord {
            timestamp: state.timestamp,
            manifest: state.manifest.clone(),
            commit: state.commit.clone(),
            content: state.content.clone(),
        },
        None => DeployRecord {
            timestamp: crate::deploy::now_secs(),
            manifest: Vec::new(),
            commit: None,
            content: crate::snapshot::Manifest::new(),
        },
    };
    store.save_deploy_record(&project.id, &record)?;

    // Checkpoint the imported state in git.
    {
        let local = local.clone();
        let message = match mode {
            PullMode::InitialImport => "Initial import from server".to_string(),
            PullMode::Sync => "Sync from server (deployed on another device)".to_string(),
        };
        tokio::task::spawn_blocking(move || -> Result<(), Error> {
            git::ensure_repo(&local)?;
            if git::status(&local)?.dirty {
                git::commit_all(&local, &message)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::Deploy(format!("git task failed: {e}")))??;
    }

    Ok((files, 0))
}

/// Download the current contents of the project's target directory into `dest`
/// as a full tree (Feather's state marker excluded). Used to snapshot the
/// deployed state for rollback — no local folder, no progress, no git. Returns
/// the number of files extracted.
pub async fn download_server_tree(
    client: &PanelClient,
    project: &ProjectConfig,
    dest: &Path,
) -> Result<usize, Error> {
    let root = normalize_target_dir(&project.target_dir)?;
    let entries: Vec<String> = client
        .list_files(&project.server_identifier, &root)
        .await?
        .into_iter()
        .map(|e| e.name)
        .filter(|name| name != STATE_FILE)
        .collect();
    if entries.is_empty() {
        return Ok(0);
    }
    let archive = client
        .compress_files(&project.server_identifier, &root, &entries)
        .await?;
    let archive_path = if root == "/" {
        format!("/{}", archive.name)
    } else {
        format!("{root}/{}", archive.name)
    };
    let signed = client
        .download_url(&project.server_identifier, &archive_path)
        .await?;
    let bytes = client.download_bytes(&signed, |_, _| {}).await?;
    // Best effort — a leftover archive is ugly but harmless.
    let _ = client
        .delete_files(
            &project.server_identifier,
            &root,
            std::slice::from_ref(&archive.name),
        )
        .await;
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || extract_tar_gz(&bytes, &dest))
        .await
        .map_err(|e| Error::Deploy(format!("extract task failed: {e}")))?
}

/// True when the folder contains anything besides a `.git` directory.
fn dir_has_content(dir: &Path) -> Result<bool, Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() != ".git" {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Extract a tar.gz into `dest`, skipping the state marker and anything that
/// would escape the destination.
fn extract_tar_gz(bytes: &[u8], dest: &Path) -> Result<usize, Error> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut files = 0usize;
    for entry in archive
        .entries()
        .map_err(|e| Error::Deploy(format!("read archive: {e}")))?
    {
        let mut entry = entry.map_err(|e| Error::Deploy(format!("read archive entry: {e}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .map_err(|e| Error::Deploy(format!("archive entry path: {e}")))?
            .into_owned();
        let mut safe = true;
        let mut target = dest.to_path_buf();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(part) => target.push(part),
                _ => {
                    safe = false;
                    break;
                }
            }
        }
        if !safe || rel.file_name().is_some_and(|n| n == STATE_FILE) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        std::fs::write(&target, contents)?;
        files += 1;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::sync_conflict;
    use crate::snapshot::Manifest;

    fn m(pairs: &[(&str, &str)]) -> Manifest {
        pairs
            .iter()
            .map(|(p, h)| (p.to_string(), h.to_string()))
            .collect()
    }

    #[test]
    fn no_conflict_when_local_matches_the_deploy() {
        let base = m(&[("a", "1"), ("b", "1")]);
        let remote = m(&[("a", "2"), ("b", "1")]); // deploy changed a
        let local = m(&[("a", "2"), ("b", "1")]); // already at the deploy
        assert!(!sync_conflict(&base, &remote, &local));
    }

    #[test]
    fn no_conflict_when_only_deploy_changed_files_differ() {
        // Deploy changes `a`; the device is still at the old `a` (behind, not
        // edited). That is the normal "just download it" case.
        let base = m(&[("a", "1"), ("b", "1")]);
        let remote = m(&[("a", "2"), ("b", "1")]);
        let local = m(&[("a", "1"), ("b", "1")]);
        assert!(!sync_conflict(&base, &remote, &local));
    }

    #[test]
    fn conflict_when_local_edited_a_file_the_deploy_left_alone() {
        // Deploy changes `a`; the device has an un-deployed edit to `b`, which
        // the deploy does not touch → pulling would lose it.
        let base = m(&[("a", "1"), ("b", "1")]);
        let remote = m(&[("a", "2"), ("b", "1")]);
        let local = m(&[("a", "1"), ("b", "local")]);
        assert!(sync_conflict(&base, &remote, &local));
    }

    #[test]
    fn no_conflict_when_local_edit_is_on_a_file_the_deploy_also_changes() {
        // Both edited `a`; the deploy governs it, so it is replaced (per spec).
        let base = m(&[("a", "1")]);
        let remote = m(&[("a", "2")]);
        let local = m(&[("a", "local")]);
        assert!(!sync_conflict(&base, &remote, &local));
    }

    #[test]
    fn local_only_files_never_block() {
        // The pull only writes files the deploy carries; a local-only file is
        // left untouched, so it is not a conflict.
        let base = m(&[("a", "1")]);
        let remote = m(&[("a", "1")]);
        let local = m(&[("a", "1"), ("scratch.txt", "mine")]);
        assert!(!sync_conflict(&base, &remote, &local));
    }

    #[test]
    fn unknown_baseline_blocks_any_overwrite() {
        // With no baseline we cannot tell edits from deploy changes, so any file
        // the pull would overwrite with different content is treated as unsafe.
        let base = Manifest::new();
        let remote = m(&[("a", "2")]);
        let local = m(&[("a", "1")]);
        assert!(sync_conflict(&base, &remote, &local));
        // …but a pull that only adds files is still fine.
        let local_ok = m(&[("a", "2")]);
        assert!(!sync_conflict(&base, &remote, &local_ok));
    }
}
