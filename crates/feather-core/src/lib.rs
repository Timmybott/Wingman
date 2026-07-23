//! Core logic for Feather, the desktop client for Pterodactyl.
//!
//! This crate is intentionally free of any Tauri dependency so it can be
//! developed and tested headless (CI, cloud environments). The Tauri shell in
//! `src-tauri` is a thin layer over the types and functions exported here.

pub mod api;
pub mod config;
pub mod deploy;
pub mod error;
pub mod git;
pub mod models;
pub mod snapshot;
pub mod storage;
pub mod sync;
pub mod ws;

pub use api::{normalize_base_url, PanelClient};
pub use config::{ConfigStore, DeployRecord, PanelConfig, PostDeployAction, ProjectConfig};
pub use deploy::{start_deploy, start_rollback, start_snapshot_rollback, DeployHandle, DeployStep};
pub use error::Error;
pub use git::{ChangedFile, CommitInfo, RepoStatus};
pub use snapshot::{
    delta_zip, diff_against, diff_manifests, manifest_of, materialize_deltas, snapshot_file,
    snapshot_zip, upload_snapshot, ChangeKind, CommitDelta, Diff, FileChange, Manifest,
};
pub use sync::{read_remote_state, start_pull, PullMode, RemoteState};
pub use ws::{Outgoing, ServerEvent, ServerSocket};
