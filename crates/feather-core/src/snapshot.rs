//! Content snapshots and diffs for cloud commits (M22).
//!
//! A **commit** captures the project's working tree as a zip stored on the
//! storage backend, plus a lightweight **manifest** (path → content hash) used
//! to diff one state against another cheaply — local vs. what's on the server,
//! or one commit vs. another — without downloading both archives.
//!
//! The scan reuses [`crate::deploy::scan_project`], so `.deployignore`, `.git/`
//! and `.deployignore` itself are excluded exactly as a deploy would pack them.

use crate::deploy::scan_project;
use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// path → content hash. `BTreeMap` keeps it sorted and deterministic.
pub type Manifest = BTreeMap<String, String>;

/// Whether a path was added, changed or removed relative to a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

/// One changed path in a [`Diff`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change: ChangeKind,
}

/// The set of changes from a base state to a new one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    pub changes: Vec<FileChange>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// `(added, modified, deleted)` counts.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut added = 0;
        let mut modified = 0;
        let mut deleted = 0;
        for c in &self.changes {
            match c.change {
                ChangeKind::Added => added += 1,
                ChangeKind::Modified => modified += 1,
                ChangeKind::Deleted => deleted += 1,
            }
        }
        (added, modified, deleted)
    }
}

/// FNV-1a 64-bit — dependency-free and deterministic. Not cryptographic; it
/// only needs to answer "did this file change", where collisions are
/// vanishingly unlikely for real project files.
fn hash_bytes(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn local_path(root: &Path, rel: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in rel.split('/') {
        path.push(segment);
    }
    path
}

/// Scan `root` into a path → content-hash manifest (ignore-aware).
pub fn manifest_of(root: &Path) -> Result<Manifest, Error> {
    let mut manifest = Manifest::new();
    for rel in scan_project(root)? {
        let bytes = std::fs::read(local_path(root, &rel))?;
        manifest.insert(rel, hash_bytes(&bytes));
    }
    Ok(manifest)
}

/// Diff a `base` manifest (e.g. what's currently on the server) against a
/// `next` one (e.g. the local working tree). Result is sorted by path.
pub fn diff_manifests(base: &Manifest, next: &Manifest) -> Diff {
    let mut changes = Vec::new();
    for (path, hash) in next {
        match base.get(path) {
            None => changes.push(FileChange {
                path: path.clone(),
                change: ChangeKind::Added,
            }),
            Some(base_hash) if base_hash != hash => changes.push(FileChange {
                path: path.clone(),
                change: ChangeKind::Modified,
            }),
            Some(_) => {}
        }
    }
    for path in base.keys() {
        if !next.contains_key(path) {
            changes.push(FileChange {
                path: path.clone(),
                change: ChangeKind::Deleted,
            });
        }
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    Diff { changes }
}

/// Diff the local working tree at `root` against a `base` manifest.
pub fn diff_against(root: &Path, base: &Manifest) -> Result<Diff, Error> {
    Ok(diff_manifests(base, &manifest_of(root)?))
}

/// Pack the working tree at `root` (ignore-aware) into an in-memory zip and
/// return the zip bytes together with the manifest it captured. The bytes are
/// uploaded to the storage backend as the commit's snapshot; the manifest is
/// kept for fast diffs.
pub fn snapshot_zip(root: &Path) -> Result<(Vec<u8>, Manifest), Error> {
    let files = scan_project(root)?;
    let mut manifest = Manifest::new();
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for rel in &files {
        let bytes = std::fs::read(local_path(root, rel))?;
        manifest.insert(rel.clone(), hash_bytes(&bytes));
        zip.start_file(rel.clone(), options)
            .map_err(|e| Error::Deploy(format!("zip {rel}: {e}")))?;
        zip.write_all(&bytes)?;
    }
    let cursor = zip
        .finish()
        .map_err(|e| Error::Deploy(format!("finish snapshot zip: {e}")))?;
    Ok((cursor.into_inner(), manifest))
}

/// Pack the working tree at `root` and upload it to the storage backend
/// through the `feather-storage` Edge Function, which holds the storage
/// server's key. Returns the file count and manifest of what was uploaded.
///
/// `endpoint` is the function URL (`…/functions/v1/feather-storage`); `token`
/// is the caller's Supabase session access token and `anon_key` the project's
/// anon key — the function authorizes the caller and derives the path itself
/// from `project_id`/`commit_id`, so no path is ever sent.
#[allow(clippy::too_many_arguments)]
pub async fn upload_snapshot(
    root: &Path,
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    commit_id: &str,
    kind: &str,
) -> Result<(usize, Manifest), Error> {
    let (bytes, manifest) = snapshot_zip(root)?;
    let files = manifest.len();
    let mut url = url::Url::parse(endpoint).map_err(|e| Error::Deploy(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("action", "put")
        .append_pair("project_id", project_id)
        .append_pair("commit_id", commit_id)
        .append_pair("kind", kind);
    let res = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("apikey", anon_key)
        .header("content-type", "application/octet-stream")
        .body(bytes)
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(Error::Deploy(format!(
            "snapshot upload failed: HTTP {}",
            res.status().as_u16()
        )));
    }
    Ok((files, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = local_path(root, rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn manifest_captures_files_and_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", "hello");
        write(dir.path(), "sub/b.txt", "world");
        let m1 = manifest_of(dir.path()).unwrap();
        assert_eq!(m1.len(), 2);
        assert!(m1.contains_key("a.txt"));
        assert!(m1.contains_key("sub/b.txt"));

        write(dir.path(), "a.txt", "hello!"); // changed
        let m2 = manifest_of(dir.path()).unwrap();
        assert_ne!(m1["a.txt"], m2["a.txt"]);
        assert_eq!(m1["sub/b.txt"], m2["sub/b.txt"]); // unchanged
    }

    #[test]
    fn diff_detects_add_modify_delete() {
        let mut base = Manifest::new();
        base.insert("keep.txt".into(), "h1".into());
        base.insert("change.txt".into(), "h2".into());
        base.insert("gone.txt".into(), "h3".into());
        let mut next = Manifest::new();
        next.insert("keep.txt".into(), "h1".into());
        next.insert("change.txt".into(), "h2-new".into());
        next.insert("new.txt".into(), "h4".into());

        let diff = diff_manifests(&base, &next);
        assert_eq!(diff.counts(), (1, 1, 1));
        assert!(diff.changes.contains(&FileChange {
            path: "new.txt".into(),
            change: ChangeKind::Added
        }));
        assert!(diff.changes.contains(&FileChange {
            path: "change.txt".into(),
            change: ChangeKind::Modified
        }));
        assert!(diff.changes.contains(&FileChange {
            path: "gone.txt".into(),
            change: ChangeKind::Deleted
        }));
    }

    #[test]
    fn identical_manifests_have_empty_diff() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", "x");
        let m = manifest_of(dir.path()).unwrap();
        assert!(diff_manifests(&m, &m).is_empty());
    }

    #[test]
    fn snapshot_zip_matches_manifest_and_is_readable() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", "hello");
        write(dir.path(), "sub/b.txt", "world");
        let (bytes, manifest) = snapshot_zip(dir.path()).unwrap();
        assert_eq!(manifest, manifest_of(dir.path()).unwrap());

        let reader = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names: Vec<String> = reader.file_names().map(String::from).collect();
        names.sort();
        assert_eq!(names, vec!["a.txt".to_string(), "sub/b.txt".to_string()]);
    }
}
