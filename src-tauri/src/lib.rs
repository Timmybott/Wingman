//! Thin Tauri shell over `wingman-core`: window setup, managed state and the
//! IPC commands the Svelte frontend calls.

mod commands;
mod secrets;

use commands::SocketHandle;
use std::collections::HashMap;
use tauri::Manager;
use wingman_core::ConfigStore;

pub struct AppState {
    store: ConfigStore,
    /// One live websocket per subscribed server, keyed by identifier.
    sockets: tokio::sync::Mutex<HashMap<String, SocketHandle>>,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(AppState {
                store: ConfigStore::new(dir),
                sockets: tokio::sync::Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_panel,
            commands::test_connection,
            commands::save_panel,
            commands::remove_panel,
            commands::list_servers,
            commands::server_resources,
            commands::set_power,
            commands::subscribe_server,
            commands::unsubscribe_server,
            commands::send_console_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
