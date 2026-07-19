//! Persistent app configuration: panels and projects as JSON files in the
//! app config directory. API keys are NOT stored here — they live in the OS
//! keychain, keyed by panel id (handled by the Tauri shell).

use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A connected panel. Stored as a list even though v1 supports a single
/// panel, so multi-panel support later is a UI change, not a migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

impl PanelConfig {
    pub fn new(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            base_url: base_url.into(),
        }
    }
}

/// Behavior after a successful deploy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostDeployAction {
    Restart,
    Notify,
}

/// A local project linked to a server. Used from M3 on; defined now so the
/// on-disk format is settled early.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub local_path: PathBuf,
    pub panel_id: String,
    pub server_uuid: String,
    /// Deploy target relative to the server root; empty = server root.
    #[serde(default)]
    pub target_dir: String,
    /// Optional command run before packing (e.g. `npm run build`).
    #[serde(default)]
    pub build_command: Option<String>,
    pub post_deploy: PostDeployAction,
    #[serde(default = "default_true")]
    pub auto_backup: bool,
}

fn default_true() -> bool {
    true
}

/// Loads and saves the JSON config files. The directory is injected so the
/// Tauri shell can pass the platform config dir and tests can use a temp dir.
pub struct ConfigStore {
    dir: PathBuf,
}

impl ConfigStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn load_panels(&self) -> Result<Vec<PanelConfig>, Error> {
        self.load_list("panels.json")
    }

    pub fn save_panels(&self, panels: &[PanelConfig]) -> Result<(), Error> {
        self.save_list("panels.json", panels)
    }

    pub fn load_projects(&self) -> Result<Vec<ProjectConfig>, Error> {
        self.load_list("projects.json")
    }

    pub fn save_projects(&self, projects: &[ProjectConfig]) -> Result<(), Error> {
        self.save_list("projects.json", projects)
    }

    fn load_list<T: serde::de::DeserializeOwned>(&self, file: &str) -> Result<Vec<T>, Error> {
        let path = self.dir.join(file);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&path).map_err(|e| Error::Config(format!("read {file}: {e}")))?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Config(format!("parse {file}: {e}")))
    }

    /// Write via temp file + rename so a crash mid-write can't corrupt the config.
    fn save_list<T: Serialize>(&self, file: &str, items: &[T]) -> Result<(), Error> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| Error::Config(format!("create config dir: {e}")))?;
        let json = serde_json::to_vec_pretty(items)
            .map_err(|e| Error::Config(format!("serialize {file}: {e}")))?;
        let tmp = self.dir.join(format!("{file}.tmp"));
        let path = self.dir.join(file);
        fs::write(&tmp, json).map_err(|e| Error::Config(format!("write {file}: {e}")))?;
        fs::rename(&tmp, &path).map_err(|e| Error::Config(format!("write {file}: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_load_as_empty_lists() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(dir.path());
        assert!(store.load_panels().unwrap().is_empty());
        assert!(store.load_projects().unwrap().is_empty());
    }

    #[test]
    fn panels_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(dir.path());
        let panel = PanelConfig::new("My Panel", "https://panel.example.com/");
        store.save_panels(std::slice::from_ref(&panel)).unwrap();
        let loaded = store.load_panels().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, panel.id);
        assert_eq!(loaded[0].base_url, "https://panel.example.com/");
    }
}
