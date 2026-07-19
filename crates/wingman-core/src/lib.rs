//! Core logic for Wingman, the desktop client for Pterodactyl.
//!
//! This crate is intentionally free of any Tauri dependency so it can be
//! developed and tested headless (CI, cloud environments). The Tauri shell in
//! `src-tauri` is a thin layer over the types and functions exported here.

pub mod api;
pub mod config;
pub mod error;
pub mod models;

pub use api::{normalize_base_url, PanelClient};
pub use config::{ConfigStore, PanelConfig, PostDeployAction, ProjectConfig};
pub use error::Error;
