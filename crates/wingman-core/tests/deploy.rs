//! Integration tests: the deploy engine against the mock panel's virtual
//! filesystem.

use mock_panel::{MockPanel, API_KEY};
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;
use wingman_core::deploy::{start_deploy, DeployStep};
use wingman_core::models::PowerState;
use wingman_core::{ConfigStore, PanelClient, PostDeployAction, ProjectConfig};

const SERVER: &str = "a1b2c3d4";

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// A small realistic project: app files, ignored build artifacts, git dir.
fn sample_project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "index.js", "console.log('hi')");
    write_file(dir.path(), "config/settings.yml", "motd: hello");
    write_file(dir.path(), "node_modules/lib/x.js", "x");
    write_file(dir.path(), ".git/HEAD", "ref: refs/heads/main");
    write_file(dir.path(), ".deployignore", "node_modules/\n");
    dir
}

fn project(local: &Path, target_dir: &str, post_deploy: PostDeployAction) -> ProjectConfig {
    ProjectConfig {
        id: "11111111-2222-3333-4444-555555555555".into(),
        name: "Test Project".into(),
        local_path: local.to_path_buf(),
        panel_id: "panel-1".into(),
        server_identifier: SERVER.into(),
        target_dir: target_dir.into(),
        build_command: None,
        post_deploy,
        auto_backup: false,
    }
}

/// Drive a deploy to its terminal event, collecting every step on the way.
async fn run_deploy(
    panel: &MockPanel,
    store: &ConfigStore,
    project: &ProjectConfig,
) -> Vec<DeployStep> {
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    let mut handle = start_deploy(client, store.clone(), project.clone());
    let mut steps = Vec::new();
    loop {
        let step = timeout(Duration::from_secs(15), handle.events.recv())
            .await
            .expect("timed out waiting for deploy event")
            .expect("deploy event channel closed before a terminal event");
        let terminal = matches!(step, DeployStep::Done { .. } | DeployStep::Failed { .. });
        steps.push(step);
        if terminal {
            return steps;
        }
    }
}

fn assert_done(steps: &[DeployStep]) -> (usize, usize) {
    match steps.last() {
        Some(DeployStep::Done { files, deleted }) => (*files, *deleted),
        other => panic!("expected Done as final step, got {other:?}"),
    }
}

#[tokio::test]
async fn deploys_to_server_root_honoring_deployignore() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());

    let steps = run_deploy(
        &panel,
        &store,
        &project(dir.path(), "", PostDeployAction::Notify),
    )
    .await;
    let (files, deleted) = assert_done(&steps);
    assert_eq!(files, 2);
    assert_eq!(deleted, 0);

    assert_eq!(
        panel.server_files(SERVER),
        vec!["config/settings.yml", "index.js"],
        "only non-ignored files, and the uploaded archive is cleaned up"
    );
    assert_eq!(
        panel.file_contents(SERVER, "index.js").unwrap(),
        b"console.log('hi')"
    );
}

#[tokio::test]
async fn deploys_into_a_subdirectory() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());

    let steps = run_deploy(
        &panel,
        &store,
        &project(dir.path(), "app", PostDeployAction::Notify),
    )
    .await;
    assert_done(&steps);

    assert_eq!(
        panel.server_files(SERVER),
        vec!["app/config/settings.yml", "app/index.js"]
    );
}

#[tokio::test]
async fn second_deploy_deletes_files_removed_locally() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let project = project(dir.path(), "", PostDeployAction::Notify);

    assert_done(&run_deploy(&panel, &store, &project).await);
    assert_eq!(
        panel.server_files(SERVER),
        vec!["config/settings.yml", "index.js"]
    );

    std::fs::remove_file(dir.path().join("config/settings.yml")).unwrap();
    let steps = run_deploy(&panel, &store, &project).await;
    let (files, deleted) = assert_done(&steps);
    assert_eq!(files, 1);
    assert_eq!(deleted, 1, "settings.yml was in the last manifest");

    assert_eq!(
        panel.server_files(SERVER),
        vec!["index.js"],
        "the locally deleted file is gone remotely too"
    );
}

#[tokio::test]
async fn emits_progress_steps_in_order() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());

    let steps = run_deploy(
        &panel,
        &store,
        &project(dir.path(), "", PostDeployAction::Notify),
    )
    .await;

    let kinds: Vec<&'static str> = steps
        .iter()
        .map(|s| match s {
            DeployStep::Scanning => "scanning",
            DeployStep::Packing { .. } => "packing",
            DeployStep::Uploading { .. } => "uploading",
            DeployStep::Extracting => "extracting",
            DeployStep::CleaningUp => "cleaning_up",
            DeployStep::Restarting => "restarting",
            DeployStep::Done { .. } => "done",
            DeployStep::Failed { .. } => "failed",
        })
        .collect();
    // Uploading may appear several times (progress); dedup for the order check.
    let mut order = kinds.clone();
    order.dedup();
    assert_eq!(
        order,
        vec![
            "scanning",
            "packing",
            "uploading",
            "extracting",
            "cleaning_up",
            "done"
        ],
        "full sequence was: {kinds:?}"
    );
}

#[tokio::test]
async fn restart_mode_triggers_a_power_cycle() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());

    let steps = run_deploy(
        &panel,
        &store,
        &project(dir.path(), "", PostDeployAction::Restart),
    )
    .await;
    assert!(
        steps.iter().any(|s| matches!(s, DeployStep::Restarting)),
        "expected a Restarting step, got {steps:?}"
    );
    assert_done(&steps);

    // The mock's restart goes stopping → starting → running; catching any
    // non-running state within the window proves the cycle happened.
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    let mut saw_transition = false;
    for _ in 0..30 {
        let stats = client.server_resources(SERVER).await.unwrap();
        if stats.current_state != PowerState::Running {
            saw_transition = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(saw_transition, "server never left the running state");
}

#[tokio::test]
async fn failing_deploy_ends_with_failed_event() {
    let panel = MockPanel::spawn().await;
    let dir = tempfile::tempdir().unwrap(); // empty project
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());

    let steps = run_deploy(
        &panel,
        &store,
        &project(dir.path(), "", PostDeployAction::Notify),
    )
    .await;
    match steps.last() {
        Some(DeployStep::Failed { message }) => {
            assert!(message.contains("nothing to deploy"), "message: {message}")
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}
