//! Thin Tauri shell over `wingman-core`: window setup, managed state and the
//! IPC commands the Svelte frontend calls.

mod commands;

use commands::{ActivePanel, SocketHandle};
use std::collections::HashMap;
use tauri::Manager;
use wingman_core::ConfigStore;

pub struct AppState {
    store: ConfigStore,
    /// The panel connected for this session. Credentials come from the cloud
    /// (decrypted per team member) and are held in memory only — never on
    /// local disk. `None` until the frontend activates a panel.
    active_panel: std::sync::Mutex<Option<ActivePanel>>,
    /// One live websocket per subscribed server, keyed by identifier.
    sockets: tokio::sync::Mutex<HashMap<String, SocketHandle>>,
    /// Project ids with a deploy in flight — guards against double deploys.
    deploys: tokio::sync::Mutex<std::collections::HashSet<String>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(AppState {
                store: ConfigStore::new(dir),
                active_panel: std::sync::Mutex::new(None),
                sockets: tokio::sync::Mutex::new(HashMap::new()),
                deploys: tokio::sync::Mutex::new(std::collections::HashSet::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
            commands::set_active_panel,
            commands::clear_active_panel,
            commands::list_servers,
            commands::server_resources,
            commands::set_power,
            commands::subscribe_server,
            commands::unsubscribe_server,
            commands::send_console_command,
            commands::list_projects,
            commands::save_project,
            commands::delete_project,
            commands::deploy_project,
            commands::rollback_project,
            commands::pull_project,
            commands::check_remote_deploy,
            commands::repo_status,
            commands::commit_project,
            commands::project_history,
            commands::deploy_status,
            commands::list_server_files,
            commands::delete_server_files,
            commands::create_server_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
