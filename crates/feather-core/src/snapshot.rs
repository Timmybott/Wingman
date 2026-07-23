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

/// A commit's stored delta: the zip of its changed (added or modified) files
/// and the paths it removed relative to the previous state. This is what a
/// commit records in the new model — only what changed, not the whole tree.
#[derive(Debug, Clone)]
pub struct CommitDelta {
    pub zip: Vec<u8>,
    pub deleted: Vec<String>,
}

/// Pack only the files that changed relative to `base` (added or modified) into
/// an in-memory zip, and report the full resulting manifest plus the paths
/// deleted relative to `base`. This is a **commit delta** — the minimal content
/// needed to move a tree from `base` to the working tree at `root`. Unlike
/// [`snapshot_zip`], unchanged files are not stored.
pub fn delta_zip(root: &Path, base: &Manifest) -> Result<(Vec<u8>, Manifest, Vec<String>), Error> {
    let resulting = manifest_of(root)?;
    let diff = diff_manifests(base, &resulting);
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut deleted = Vec::new();
    for change in &diff.changes {
        match change.change {
            ChangeKind::Added | ChangeKind::Modified => {
                let bytes = std::fs::read(local_path(root, &change.path))?;
                zip.start_file(change.path.clone(), options)
                    .map_err(|e| Error::Deploy(format!("zip {}: {e}", change.path)))?;
                zip.write_all(&bytes)?;
            }
            ChangeKind::Deleted => deleted.push(change.path.clone()),
        }
    }
    let cursor = zip
        .finish()
        .map_err(|e| Error::Deploy(format!("finish delta zip: {e}")))?;
    Ok((cursor.into_inner(), resulting, deleted))
}

/// The file entry names stored in a zip (our own archives; `..` rejected).
fn zip_entry_names(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Deploy(format!("open delta: {e}")))?;
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| Error::Deploy(format!("read delta entry: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.contains("..") {
            continue;
        }
        names.push(name);
    }
    Ok(names)
}

/// Overlay an ordered chain of commit deltas on top of `base` into `dest`
/// (oldest first; a newer commit's file overwrites an older one). `dest` ends
/// up containing exactly the added/modified files of the net change — what must
/// be written to a server currently at `base` to reach the committed state.
///
/// Returns `(net_deleted, resulting)`: the paths to delete on that server
/// (present in `base`, gone after the whole chain) and the resulting manifest
/// (the new full server state). A deploy is thus purely the sum of its commits —
/// it introduces no changes of its own.
pub fn materialize_deltas(
    base: &Manifest,
    deltas: &[CommitDelta],
    dest: &Path,
) -> Result<(Vec<String>, Manifest), Error> {
    let mut manifest = base.clone();
    for delta in deltas {
        extract_zip(&delta.zip, dest)?;
        for name in zip_entry_names(&delta.zip)? {
            let bytes = std::fs::read(local_path(dest, &name))?;
            manifest.insert(name, hash_bytes(&bytes));
        }
        for path in &delta.deleted {
            manifest.remove(path);
            // A file added by an earlier commit and removed by a later one must
            // not be uploaded, so drop it from the staging tree too.
            let _ = std::fs::remove_file(local_path(dest, path));
        }
    }
    let net_deleted = base
        .keys()
        .filter(|p| !manifest.contains_key(*p))
        .cloned()
        .collect();
    Ok((net_deleted, manifest))
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
    put_snapshot(
        bytes, endpoint, token, anon_key, project_id, commit_id, kind,
    )
    .await?;
    Ok((files, manifest))
}

/// Pack only what changed relative to `base` (a **commit delta**) and upload it
/// to the storage backend. Returns the number of changed paths (added,
/// modified and deleted) and the full resulting manifest of the working tree —
/// the latter is recorded as the commit's state so a deploy can apply the
/// bundle. See [`upload_snapshot`] for the auth model.
#[allow(clippy::too_many_arguments)]
pub async fn upload_delta(
    root: &Path,
    base: &Manifest,
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    commit_id: &str,
    kind: &str,
) -> Result<(usize, Manifest), Error> {
    let (bytes, resulting, deleted) = delta_zip(root, base)?;
    // Changed paths = added/modified (in the zip) + deleted.
    let changed = zip_entry_names(&bytes)?.len() + deleted.len();
    put_snapshot(
        bytes, endpoint, token, anon_key, project_id, commit_id, kind,
    )
    .await?;
    Ok((changed, resulting))
}

/// POST a snapshot/delta zip to the storage backend through the Edge Function.
async fn put_snapshot(
    bytes: Vec<u8>,
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    commit_id: &str,
    kind: &str,
) -> Result<(), Error> {
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
    Ok(())
}

/// Download a commit/rollback snapshot (a zip) from the storage backend through
/// the `feather-storage` Edge Function. See [`upload_snapshot`] for the auth
/// model — the function derives the path from the ids.
#[allow(clippy::too_many_arguments)]
pub async fn download_snapshot(
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    commit_id: &str,
    kind: &str,
) -> Result<Vec<u8>, Error> {
    let mut url = url::Url::parse(endpoint).map_err(|e| Error::Deploy(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("action", "get")
        .append_pair("project_id", project_id)
        .append_pair("commit_id", commit_id)
        .append_pair("kind", kind);
    let res = reqwest::Client::new()
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("apikey", anon_key)
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(Error::Deploy(format!(
            "snapshot download failed: HTTP {}",
            res.status().as_u16()
        )));
    }
    Ok(res.bytes().await?.to_vec())
}

/// Download a commit/rollback snapshot and return one file's UTF-8 text, or an
/// empty string if that path isn't in the snapshot (e.g. it was added later).
#[allow(clippy::too_many_arguments)]
pub async fn snapshot_file(
    endpoint: &str,
    token: &str,
    anon_key: &str,
    project_id: &str,
    commit_id: &str,
    kind: &str,
    path: &str,
) -> Result<Option<String>, Error> {
    let bytes = download_snapshot(endpoint, token, anon_key, project_id, commit_id, kind).await?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Deploy(format!("open snapshot: {e}")))?;
    let mut buf = Vec::new();
    let found = match archive.by_name(path) {
        Ok(mut entry) => {
            std::io::copy(&mut entry, &mut buf)?;
            true
        }
        Err(_) => false,
    };
    // `None` = the path is not in this snapshot's zip. For a delta that means
    // the commit did not add/modify this file (it was inherited or removed), so
    // callers can walk back to the commit that actually wrote it.
    if found {
        Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
    } else {
        Ok(None)
    }
}

/// Extract a snapshot zip into `dest`. Entry paths are sanitized (a malicious
/// archive can never write outside `dest`).
pub fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), Error> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Deploy(format!("open snapshot: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| Error::Deploy(format!("read snapshot entry: {e}")))?;
        // enclosed_name() rejects absolute paths and `..` traversal.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut file)?;
        }
    }
    Ok(())
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
    fn snapshot_zip_then_extract_roundtrips() {
        let src = tempfile::tempdir().unwrap();
        write(src.path(), "a.txt", "hello");
        write(src.path(), "sub/b.txt", "world");
        let (bytes, _) = snapshot_zip(src.path()).unwrap();

        let dest = tempfile::tempdir().unwrap();
        extract_zip(&bytes, dest.path()).unwrap();
        assert_eq!(
            fs::read_to_string(dest.path().join("a.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            fs::read_to_string(dest.path().join("sub/b.txt")).unwrap(),
            "world"
        );
        // Re-scanning the extracted tree yields the same manifest.
        assert_eq!(
            manifest_of(src.path()).unwrap(),
            manifest_of(dest.path()).unwrap()
        );
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

    /// Build a CommitDelta from a directory `root` against a `base` manifest.
    fn delta_of(root: &Path, base: &Manifest) -> (CommitDelta, Manifest) {
        let (zip, resulting, deleted) = delta_zip(root, base).unwrap();
        (CommitDelta { zip, deleted }, resulting)
    }

    fn names_in(zip: &[u8]) -> Vec<String> {
        let mut n = zip_entry_names(zip).unwrap();
        n.sort();
        n
    }

    #[test]
    fn delta_zip_packs_only_changed_files_and_lists_deletions() {
        // Base state.
        let base_dir = tempfile::tempdir().unwrap();
        write(base_dir.path(), "keep.txt", "same");
        write(base_dir.path(), "change.txt", "old");
        write(base_dir.path(), "gone.txt", "bye");
        let base = manifest_of(base_dir.path()).unwrap();

        // Working tree: keep unchanged, change modified, gone deleted, new added.
        let work = tempfile::tempdir().unwrap();
        write(work.path(), "keep.txt", "same");
        write(work.path(), "change.txt", "new");
        write(work.path(), "new.txt", "fresh");

        let (zip, resulting, deleted) = delta_zip(work.path(), &base).unwrap();
        // Only the added/modified files are stored — not the unchanged one.
        assert_eq!(names_in(&zip), vec!["change.txt", "new.txt"]);
        assert_eq!(deleted, vec!["gone.txt"]);
        assert_eq!(resulting, manifest_of(work.path()).unwrap());
    }

    #[test]
    fn materialize_combines_commits_touching_different_files() {
        // Server baseline has one shared file.
        let base_dir = tempfile::tempdir().unwrap();
        write(base_dir.path(), "shared.txt", "s");
        let base = manifest_of(base_dir.path()).unwrap();

        // Commit A adds x.txt (keeps shared unchanged).
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "shared.txt", "s");
        write(a.path(), "x.txt", "xx");
        let (da, _) = delta_of(a.path(), &base);

        // Commit B adds y.txt.
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "shared.txt", "s");
        write(b.path(), "y.txt", "yy");
        let (db, _) = delta_of(b.path(), &base);

        let dest = tempfile::tempdir().unwrap();
        let (net_deleted, resulting) = materialize_deltas(&base, &[da, db], dest.path()).unwrap();

        // Both members' changes land, even though each was diffed against base.
        assert_eq!(fs::read_to_string(dest.path().join("x.txt")).unwrap(), "xx");
        assert_eq!(fs::read_to_string(dest.path().join("y.txt")).unwrap(), "yy");
        // The unchanged shared file is not re-uploaded.
        assert!(!dest.path().join("shared.txt").exists());
        assert!(net_deleted.is_empty());
        assert!(resulting.contains_key("shared.txt"));
        assert!(resulting.contains_key("x.txt"));
        assert!(resulting.contains_key("y.txt"));
    }

    #[test]
    fn materialize_reports_deletions_to_apply_on_the_server() {
        let base_dir = tempfile::tempdir().unwrap();
        write(base_dir.path(), "keep.txt", "k");
        write(base_dir.path(), "gone.txt", "g");
        let base = manifest_of(base_dir.path()).unwrap();

        // Commit removes gone.txt.
        let work = tempfile::tempdir().unwrap();
        write(work.path(), "keep.txt", "k");
        let (delta, _) = delta_of(work.path(), &base);

        let dest = tempfile::tempdir().unwrap();
        let (net_deleted, resulting) = materialize_deltas(&base, &[delta], dest.path()).unwrap();

        assert_eq!(net_deleted, vec!["gone.txt"]);
        assert!(resulting.contains_key("keep.txt"));
        assert!(!resulting.contains_key("gone.txt"));
        // Nothing to upload (only a deletion happened).
        assert!(!dest.path().join("keep.txt").exists());
    }

    #[test]
    fn materialize_nets_out_a_file_added_then_deleted() {
        let base = Manifest::new(); // empty server

        // Commit A adds f.txt.
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "f.txt", "f");
        let (da, _) = delta_of(a.path(), &base);

        // Commit B (built on A's state) deletes f.txt.
        let b = tempfile::tempdir().unwrap();
        // empty dir → f.txt gone relative to A's manifest
        let a_manifest = manifest_of(a.path()).unwrap();
        let (db, _) = delta_of(b.path(), &a_manifest);

        let dest = tempfile::tempdir().unwrap();
        let (net_deleted, resulting) = materialize_deltas(&base, &[da, db], dest.path()).unwrap();

        assert!(!dest.path().join("f.txt").exists());
        assert!(resulting.is_empty());
        // f.txt never existed on the server, so nothing to delete there.
        assert!(net_deleted.is_empty());
    }
}
