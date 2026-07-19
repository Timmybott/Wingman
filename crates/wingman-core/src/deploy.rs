//! The deploy engine — Wingman's core feature.
//!
//! Flow (spec 6.3, the M3 subset): scan the project folder honoring
//! `.deployignore` → pack a zip → upload via the panel's signed URL →
//! decompress into the target directory → delete the remote archive →
//! delete files that were in the previous deploy but are gone locally
//! (manifest diff) → optionally restart the server.
//!
//! Same supervision pattern as [`crate::ws`]: `start_deploy` spawns a task
//! and hands back an event receiver the UI can render step by step.

use crate::api::PanelClient;
use crate::config::{ConfigStore, DeployRecord, PostDeployAction, ProjectConfig};
use crate::error::Error;
use crate::models::PowerSignal;
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Progress events, serialized as `{"step":…, …}` for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum DeployStep {
    Scanning,
    Packing { files: usize },
    Uploading { percent: u8 },
    Extracting,
    CleaningUp,
    Restarting,
    Done { files: usize, deleted: usize },
    Failed { message: String },
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
    let (tx, events) = mpsc::channel(64);
    tokio::spawn(async move {
        match run(&client, &store, &project, &tx).await {
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

async fn run(
    client: &PanelClient,
    store: &ConfigStore,
    project: &ProjectConfig,
    tx: &mpsc::Sender<DeployStep>,
) -> Result<(usize, usize), Error> {
    let root = normalize_target_dir(&project.target_dir)?;
    let local_path = project.local_path.clone();
    if !local_path.is_dir() {
        return Err(Error::Deploy(format!(
            "project folder does not exist: {}",
            local_path.display()
        )));
    }

    let _ = tx.send(DeployStep::Scanning).await;
    // Scanning and zipping are blocking filesystem work.
    let (manifest, archive) = {
        let local_path = local_path.clone();
        tokio::task::spawn_blocking(move || -> Result<_, Error> {
            let manifest = scan_project(&local_path)?;
            if manifest.is_empty() {
                return Err(Error::Deploy(
                    "nothing to deploy — the project folder is empty or everything is ignored"
                        .into(),
                ));
            }
            let archive = pack_zip(&local_path, &manifest)?;
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

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_secs();
    let remote_name = format!(".wingman-deploy-{timestamp}.zip");

    let _ = tx.send(DeployStep::Uploading { percent: 0 }).await;
    let signed_url = client.upload_url(&project.server_identifier).await?;
    let progress_tx = tx.clone();
    let mut last_percent = 0u8;
    client
        .upload_zip(
            &signed_url,
            &root,
            archive.path(),
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
        .decompress_file(&project.server_identifier, &root, &remote_name)
        .await?;

    let _ = tx.send(DeployStep::CleaningUp).await;
    client
        .delete_files(
            &project.server_identifier,
            &root,
            std::slice::from_ref(&remote_name),
        )
        .await?;

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

    store.save_deploy_record(
        &project.id,
        &DeployRecord {
            timestamp,
            manifest: manifest.clone(),
        },
    )?;

    if project.post_deploy == PostDeployAction::Restart {
        let _ = tx.send(DeployStep::Restarting).await;
        client
            .set_power(&project.server_identifier, PowerSignal::Restart)
            .await?;
    }

    Ok((manifest.len(), stale.len()))
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
        copy(&mut src, &mut zip)?;
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

fn copy<R: Read, W: Write + Seek>(src: &mut R, zip: &mut zip::ZipWriter<W>) -> Result<(), Error> {
    std::io::copy(src, zip)?;
    Ok(())
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
}
