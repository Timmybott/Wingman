//! API keys are stored exclusively in the OS keychain (Windows Credential
//! Manager, Secret Service on Linux) — never in plain-text config files.
//! There is deliberately no plain-text fallback.

const SERVICE: &str = "wingman";

pub fn get_api_key(panel_id: &str) -> Result<String, String> {
    entry(panel_id)?.get_password().map_err(map_err)
}

pub fn set_api_key(panel_id: &str, api_key: &str) -> Result<(), String> {
    entry(panel_id)?.set_password(api_key).map_err(map_err)
}

pub fn delete_api_key(panel_id: &str) -> Result<(), String> {
    entry(panel_id)?.delete_credential().map_err(map_err)
}

fn entry(panel_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, panel_id).map_err(map_err)
}

fn map_err(err: keyring::Error) -> String {
    #[cfg(target_os = "linux")]
    {
        format!(
            "keychain error: {err} — Wingman stores API keys only in the system keychain; \
             make sure a Secret Service is running (e.g. GNOME Keyring or KWallet)"
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!("keychain error: {err}")
    }
}
