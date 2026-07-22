//! Local git operations (libgit2 via git2, no network transports).
//!
//! Feather keeps every linked project in a plain git repository: commits are
//! deploy checkpoints, history powers rollback. Power users can work with the
//! same repo using normal git tooling — nothing here is Feather-specific.
//!
//! All functions are blocking (libgit2 is synchronous); callers on an async
//! runtime wrap them in `spawn_blocking`.

use crate::error::Error;
use git2::{ObjectType, Oid, Repository, Signature, Sort, Status, StatusOptions, TreeWalkMode};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    /// Unix seconds.
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangedFile {
    pub path: String,
    /// "new" | "modified" | "deleted" | "renamed" | "other"
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoStatus {
    pub dirty: bool,
    pub changed: Vec<ChangedFile>,
    pub head: Option<CommitInfo>,
}

fn map_err(err: git2::Error) -> Error {
    Error::Git(err.message().to_string())
}

/// Open the project repository, initializing a fresh one when none exists.
pub fn ensure_repo(path: &Path) -> Result<(), Error> {
    if Repository::open(path).is_ok() {
        return Ok(());
    }
    Repository::init(path).map_err(map_err)?;
    Ok(())
}

fn signature(repo: &Repository) -> Result<Signature<'static>, Error> {
    // Use the user's configured identity when available, otherwise a
    // neutral fallback so committing never blocks on git config.
    repo.signature()
        .or_else(|_| Signature::now("Feather", "feather@localhost"))
        .map_err(map_err)
}

fn head_commit(repo: &Repository) -> Option<git2::Commit<'_>> {
    repo.head().ok().and_then(|h| h.peel_to_commit().ok())
}

fn commit_info(commit: &git2::Commit<'_>) -> CommitInfo {
    let id = commit.id().to_string();
    CommitInfo {
        short_id: id.chars().take(8).collect(),
        id,
        summary: commit.summary().unwrap_or("").to_string(),
        author: commit.author().name().unwrap_or("unknown").to_string(),
        timestamp: commit.time().seconds(),
    }
}

/// Working-tree status (respects .gitignore).
pub fn status(path: &Path) -> Result<RepoStatus, Error> {
    let repo = Repository::open(path).map_err(map_err)?;
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut options)).map_err(map_err)?;
    let mut changed = Vec::new();
    for entry in statuses.iter() {
        let flags = entry.status();
        if flags == Status::CURRENT || flags.is_ignored() {
            continue;
        }
        let kind = if flags.intersects(Status::WT_NEW | Status::INDEX_NEW) {
            "new"
        } else if flags.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
            "deleted"
        } else if flags.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
            "renamed"
        } else if flags.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED) {
            "modified"
        } else {
            "other"
        };
        changed.push(ChangedFile {
            path: entry.path().unwrap_or("<invalid utf-8>").to_string(),
            kind,
        });
    }
    changed.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(RepoStatus {
        dirty: !changed.is_empty(),
        changed,
        head: head_commit(&repo).map(|c| commit_info(&c)),
    })
}

/// Stage everything (including deletions) and commit. Returns the new commit
/// id — or the current HEAD id unchanged when there is nothing to commit.
pub fn commit_all(path: &Path, message: &str) -> Result<CommitInfo, Error> {
    let repo = Repository::open(path).map_err(map_err)?;

    let current = status(path)?;
    if !current.dirty {
        return current
            .head
            .ok_or_else(|| Error::Git("nothing to commit in an empty repository".into()));
    }

    let mut index = repo.index().map_err(map_err)?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(map_err)?;
    index.update_all(["*"].iter(), None).map_err(map_err)?;
    index.write().map_err(map_err)?;
    let tree_id = index.write_tree().map_err(map_err)?;
    let tree = repo.find_tree(tree_id).map_err(map_err)?;
    let sig = signature(&repo)?;
    let parent = head_commit(&repo);
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(map_err)?;
    let commit = repo.find_commit(oid).map_err(map_err)?;
    Ok(commit_info(&commit))
}

/// The most recent commits, newest first. An unborn HEAD yields an empty list.
pub fn log(path: &Path, limit: usize) -> Result<Vec<CommitInfo>, Error> {
    let repo = Repository::open(path).map_err(map_err)?;
    let mut walk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(err) => return Err(map_err(err)),
    };
    if walk.push_head().is_err() {
        return Ok(Vec::new()); // No commits yet.
    }
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(map_err)?;
    let mut commits = Vec::new();
    for oid in walk.take(limit) {
        let oid = oid.map_err(map_err)?;
        let commit = repo.find_commit(oid).map_err(map_err)?;
        commits.push(commit_info(&commit));
    }
    Ok(commits)
}

/// How many commits HEAD is ahead of `since` (a commit id). If `since` is
/// unknown (e.g. rewritten history), every reachable commit counts.
pub fn commits_ahead(path: &Path, since: &str) -> Result<usize, Error> {
    let repo = Repository::open(path).map_err(map_err)?;
    let mut walk = repo.revwalk().map_err(map_err)?;
    if walk.push_head().is_err() {
        return Ok(0);
    }
    if let Ok(oid) = Oid::from_str(since) {
        let _ = walk.hide(oid); // Unknown commit: nothing hidden, count all.
    }
    Ok(walk.count())
}

/// Write the tree of `commit_id` into `dest` — like `git archive`, without
/// ever touching the working directory (rollback source, spec 6.4).
pub fn archive_commit(path: &Path, commit_id: &str, dest: &Path) -> Result<usize, Error> {
    let repo = Repository::open(path).map_err(map_err)?;
    let oid = Oid::from_str(commit_id).map_err(map_err)?;
    let commit = repo.find_commit(oid).map_err(map_err)?;
    let tree = commit.tree().map_err(map_err)?;

    let mut entries: Vec<(String, Oid)> = Vec::new();
    tree.walk(TreeWalkMode::PreOrder, |dir, entry| {
        if entry.kind() == Some(ObjectType::Blob) {
            if let Some(name) = entry.name() {
                entries.push((format!("{dir}{name}"), entry.id()));
            }
        }
        git2::TreeWalkResult::Ok
    })
    .map_err(map_err)?;

    for (rel, blob_oid) in &entries {
        let blob = repo.find_blob(*blob_oid).map_err(map_err)?;
        let mut target = dest.to_path_buf();
        for segment in rel.split('/') {
            target.push(segment);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, blob.content())?;
    }
    Ok(entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn init_commit_status_log_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        ensure_repo(dir.path()).unwrap();
        // Idempotent on an existing repo.
        ensure_repo(dir.path()).unwrap();

        write(dir.path(), "a.txt", "one");
        let st = status(dir.path()).unwrap();
        assert!(st.dirty);
        assert_eq!(st.changed[0].path, "a.txt");
        assert_eq!(st.changed[0].kind, "new");
        assert!(st.head.is_none());

        let first = commit_all(dir.path(), "first").unwrap();
        assert!(!status(dir.path()).unwrap().dirty);

        // Nothing to commit → same HEAD comes back.
        let same = commit_all(dir.path(), "noop").unwrap();
        assert_eq!(same.id, first.id);

        write(dir.path(), "a.txt", "two");
        write(dir.path(), "b.txt", "new");
        let second = commit_all(dir.path(), "second").unwrap();
        assert_ne!(second.id, first.id);

        let history = log(dir.path(), 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].summary, "second");
        assert_eq!(history[1].summary, "first");

        assert_eq!(commits_ahead(dir.path(), &first.id).unwrap(), 1);
        assert_eq!(commits_ahead(dir.path(), &second.id).unwrap(), 0);
    }

    #[test]
    fn commit_all_records_deletions() {
        let dir = tempfile::tempdir().unwrap();
        ensure_repo(dir.path()).unwrap();
        write(dir.path(), "keep.txt", "k");
        write(dir.path(), "gone.txt", "g");
        commit_all(dir.path(), "both").unwrap();

        std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
        let st = status(dir.path()).unwrap();
        assert!(st.dirty);
        assert_eq!(st.changed[0].kind, "deleted");
        commit_all(dir.path(), "delete gone").unwrap();

        let dest = tempfile::tempdir().unwrap();
        let head = log(dir.path(), 1).unwrap().remove(0);
        archive_commit(dir.path(), &head.id, dest.path()).unwrap();
        assert!(dest.path().join("keep.txt").exists());
        assert!(!dest.path().join("gone.txt").exists());
    }

    #[test]
    fn archive_restores_an_old_state_without_touching_the_worktree() {
        let dir = tempfile::tempdir().unwrap();
        ensure_repo(dir.path()).unwrap();
        write(dir.path(), "app.js", "v1");
        write(dir.path(), "sub/lib.js", "lib-v1");
        let v1 = commit_all(dir.path(), "v1").unwrap();

        write(dir.path(), "app.js", "v2");
        commit_all(dir.path(), "v2").unwrap();
        write(dir.path(), "app.js", "uncommitted");

        let dest = tempfile::tempdir().unwrap();
        let files = archive_commit(dir.path(), &v1.id, dest.path()).unwrap();
        assert_eq!(files, 2);
        assert_eq!(
            std::fs::read_to_string(dest.path().join("app.js")).unwrap(),
            "v1"
        );
        assert_eq!(
            std::fs::read_to_string(dest.path().join("sub/lib.js")).unwrap(),
            "lib-v1"
        );
        // The working tree keeps its uncommitted changes.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("app.js")).unwrap(),
            "uncommitted"
        );
    }
}
