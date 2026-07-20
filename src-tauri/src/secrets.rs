//! API key storage. The OS keychain (Windows Credential Manager, Secret
//! Service on Linux) is always tried first. On systems without a working
//! keychain — e.g. a minimal Linux setup without GNOME Keyring/KWallet —
//! the key falls back to `credentials.json` in the config directory so the
//! app remains usable. That file is hex-obfuscated, NOT encrypted; the
//! README documents this trade-off.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SERVICE: &str = "wingman";

pub fn get_api_key(config_dir: &Path, panel_id: &str) -> Result<String, String> {
    match entry(panel_id).and_then(|e| e.get_password().map_err(map_err)) {
        Ok(key) => Ok(key),
        Err(keyring_error) => file_get(config_dir, panel_id).ok_or(keyring_error),
    }
}

pub fn set_api_key(config_dir: &Path, panel_id: &str, api_key: &str) -> Result<(), String> {
    match entry(panel_id).and_then(|e| e.set_password(api_key).map_err(map_err)) {
        Ok(()) => {
            // A key may have landed in the fallback file earlier — clean it.
            let _ = file_remove(config_dir, panel_id);
            Ok(())
        }
        Err(_) => file_set(config_dir, panel_id, api_key),
    }
}

pub fn delete_api_key(config_dir: &Path, panel_id: &str) -> Result<(), String> {
    let keychain = entry(panel_id).and_then(|e| e.delete_credential().map_err(map_err));
    let file = file_remove(config_dir, panel_id);
    keychain.or(file)
}

fn entry(panel_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, panel_id).map_err(map_err)
}

fn map_err(err: keyring::Error) -> String {
    format!("keychain error: {err}")
}

// --- fallback file -------------------------------------------------------

fn credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join("credentials.json")
}

fn load_file(config_dir: &Path) -> HashMap<String, String> {
    std::fs::read(credentials_path(config_dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn store_file(config_dir: &Path, map: &HashMap<String, String>) -> Result<(), String> {
    let path = credentials_path(config_dir);
    if map.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let json = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write credentials fallback: {e}"))
}

fn file_get(config_dir: &Path, panel_id: &str) -> Option<String> {
    load_file(config_dir)
        .get(panel_id)
        .and_then(|hex| hex_decode(hex))
}

fn file_set(config_dir: &Path, panel_id: &str, api_key: &str) -> Result<(), String> {
    let mut map = load_file(config_dir);
    map.insert(panel_id.to_string(), hex_encode(api_key));
    store_file(config_dir, &map)
}

fn file_remove(config_dir: &Path, panel_id: &str) -> Result<(), String> {
    let mut map = load_file(config_dir);
    map.remove(panel_id);
    store_file(config_dir, &map)
}

fn hex_encode(value: &str) -> String {
    value.bytes().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(hex: &str) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}
