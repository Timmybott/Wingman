//! Standalone mock panel for local frontend development:
//! `cargo run -p mock-panel` and point Wingman at the printed URL.

#[tokio::main]
async fn main() {
    let panel = mock_panel::MockPanel::spawn_on("127.0.0.1:8899".parse().unwrap()).await;
    println!("Mock Pterodactyl panel listening on {}", panel.base_url());
    println!("API key: {}", mock_panel::API_KEY);
    println!("Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.expect("ctrl_c");
}
