//! Standalone mock panel for local frontend development:
//! `cargo run -p mock-panel` and point Feather at the printed URL.

use mock_panel::{MockPanel, MockPanelOptions, API_KEY};

#[tokio::main]
async fn main() {
    let panel = MockPanel::spawn_on(
        "127.0.0.1:8899".parse().unwrap(),
        MockPanelOptions::default(),
    )
    .await;
    println!("Mock Pterodactyl panel listening on {}", panel.base_url());
    println!("API key: {API_KEY}");
    println!("Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.expect("ctrl_c");
}
