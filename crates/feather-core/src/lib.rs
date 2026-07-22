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
pub mod sync;
pub mod ws;

pub use api::{normalize_base_url, PanelClient};
pub use config::{ConfigStore, DeployRecord, PanelConfig, PostDeployAction, ProjectConfig};
pub use deploy::{start_deploy, start_rollback, DeployHandle, DeployStep};
pub use error::Error;
pub use git::{ChangedFile, CommitInfo, RepoStatus};
pub use sync::{read_remote_state, start_pull, PullMode, RemoteState};
pub use ws::{Outgoing, ServerEvent, ServerSocket};
