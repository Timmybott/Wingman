//! Integration tests: wingman-core's PanelClient against the mock panel.

use mock_panel::MockPanel;
use wingman_core::models::PowerState;
use wingman_core::{Error, PanelClient};

#[tokio::test]
async fn lists_all_servers_across_pages() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), mock_panel::API_KEY).unwrap();

    // The mock paginates with per_page = 2, so 3 servers require 2 requests.
    let servers = client.list_servers().await.unwrap();
    assert_eq!(servers.len(), 3);
    assert_eq!(servers[0].name, "Survival SMP");
    assert_eq!(servers[0].identifier, "a1b2c3d4");
    assert_eq!(servers[0].limits.memory, Some(4096));
    assert_eq!(servers[0].feature_limits.backups, Some(3));
    assert_eq!(servers[2].name, "Factorio");
    assert_eq!(
        servers[2].limits.cpu,
        Some(0),
        "unlimited CPU is represented as 0"
    );
}

#[tokio::test]
async fn rejects_bad_api_key() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), "ptlc_wrong_key").unwrap();

    let err = client.list_servers().await.unwrap_err();
    assert!(
        matches!(err, Error::Unauthorized { status: 401 }),
        "expected Unauthorized, got: {err:?}"
    );
}

#[tokio::test]
async fn fetches_resources_of_a_running_server() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), mock_panel::API_KEY).unwrap();

    let stats = client.server_resources("a1b2c3d4").await.unwrap();
    assert_eq!(stats.current_state, PowerState::Running);
    assert!(stats.resources.memory_bytes > 0);
    assert!(stats.resources.cpu_absolute > 0.0);
}

#[tokio::test]
async fn reports_offline_state() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), mock_panel::API_KEY).unwrap();

    let stats = client.server_resources("c3d4e5f6").await.unwrap();
    assert_eq!(stats.current_state, PowerState::Offline);
    assert_eq!(stats.resources.uptime, 0);
}

#[tokio::test]
async fn unknown_server_is_an_api_error() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), mock_panel::API_KEY).unwrap();

    let err = client.server_resources("ffffffff").await.unwrap_err();
    match err {
        Error::Api { status, detail } => {
            assert_eq!(status, 404);
            assert!(detail.contains("NotFoundHttpException"), "detail: {detail}");
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}
