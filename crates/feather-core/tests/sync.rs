//! Integration tests for server → local sync: initial import on link and
//! multi-device sync via the remote state marker.

use feather_core::deploy::{start_deploy, DeployStep};
use feather_core::sync::{is_newer, read_remote_state, start_pull, PullMode};
use feather_core::{git, ConfigStore, PanelClient, PostDeployAction, ProjectConfig};
use mock_panel::{MockPanel, API_KEY};
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

const SERVER: &str = "a1b2c3d4";

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn project(id: &str, local: &Path) -> ProjectConfig {
    ProjectConfig {
        id: id.into(),
        name: format!("Project {id}"),
        local_path: local.to_path_buf(),
        panel_id: "panel-1".into(),
        server_identifier: SERVER.into(),
        target_dir: String::new(),
        build_command: None,
        post_deploy: PostDeployAction::Notify,
        auto_backup: false,
    }
}

async fn drive(mut handle: feather_core::DeployHandle) -> Vec<DeployStep> {
    let mut steps = Vec::new();
    loop {
        let step = timeout(Duration::from_secs(15), handle.events.recv())
            .await
            .expect("timed out waiting for engine event")
            .expect("event channel closed before a terminal event");
        let terminal = matches!(step, DeployStep::Done { .. } | DeployStep::Failed { .. });
        steps.push(step);
        if terminal {
            return steps;
        }
    }
}

fn done_files(steps: &[DeployStep]) -> usize {
    match steps.last() {
        Some(DeployStep::Done { files, .. }) => *files,
        other => panic!("expected Done, got {other:?}"),
    }
}

#[tokio::test]
async fn initial_import_pulls_server_files_into_empty_folder() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    // Files that already live on the server before Feather is involved.
    client
        .write_file(SERVER, "/server.properties", b"motd=hi".to_vec())
        .await
        .unwrap();
    client
        .write_file(SERVER, "/plugins/config.yml", b"a: 1".to_vec())
        .await
        .unwrap();

    let local = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj = project("aaaaaaaa-0000-0000-0000-000000000001", local.path());

    let steps = drive(start_pull(
        client.clone(),
        store.clone(),
        proj.clone(),
        PullMode::InitialImport,
    ))
    .await;
    assert_eq!(done_files(&steps), 2);
    assert!(steps
        .iter()
        .any(|s| matches!(s, DeployStep::Downloading { .. })));

    assert_eq!(
        std::fs::read_to_string(local.path().join("server.properties")).unwrap(),
        "motd=hi"
    );
    assert_eq!(
        std::fs::read_to_string(local.path().join("plugins/config.yml")).unwrap(),
        "a: 1"
    );
    // The import is checkpointed in git.
    let history = git::log(local.path(), 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "Initial import from server");
}

#[tokio::test]
async fn initial_import_never_overwrites_a_non_empty_folder() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    client
        .write_file(SERVER, "/server.properties", b"motd=hi".to_vec())
        .await
        .unwrap();

    let local = tempfile::tempdir().unwrap();
    write_file(local.path(), "precious.txt", "mine");
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj = project("aaaaaaaa-0000-0000-0000-000000000002", local.path());

    let steps = drive(start_pull(client, store, proj, PullMode::InitialImport)).await;
    assert_eq!(done_files(&steps), 0);
    assert!(
        steps
            .iter()
            .any(|s| matches!(s, DeployStep::BackupSkipped { .. })),
        "expected a skip note, got {steps:?}"
    );
    assert!(!local.path().join("server.properties").exists());
    assert_eq!(
        std::fs::read_to_string(local.path().join("precious.txt")).unwrap(),
        "mine"
    );
}

#[tokio::test]
async fn second_device_syncs_a_newer_deploy() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    // Device A deploys v1.
    let dir_a = tempfile::tempdir().unwrap();
    write_file(dir_a.path(), "bot.js", "v1");
    let store_a = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_a = project("aaaaaaaa-0000-0000-0000-00000000000a", dir_a.path());
    drive(start_deploy(
        client.clone(),
        store_a.clone(),
        proj_a.clone(),
    ))
    .await;

    // Device B links the same server: initial import aligns it with A.
    let dir_b = tempfile::tempdir().unwrap();
    let store_b = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_b = project("bbbbbbbb-0000-0000-0000-00000000000b", dir_b.path());
    drive(start_pull(
        client.clone(),
        store_b.clone(),
        proj_b.clone(),
        PullMode::InitialImport,
    ))
    .await;
    assert_eq!(
        std::fs::read_to_string(dir_b.path().join("bot.js")).unwrap(),
        "v1"
    );
    // B's record matches the remote state → nothing is "newer".
    let state = read_remote_state(&client, &proj_b).await.unwrap().unwrap();
    let record_b = store_b.load_deploy_record(&proj_b.id).unwrap();
    assert!(!is_newer(&state, record_b.as_ref()));

    // Device A deploys v2.
    write_file(dir_a.path(), "bot.js", "v2");
    write_file(dir_a.path(), "new.txt", "added");
    drive(start_deploy(client.clone(), store_a, proj_a)).await;

    // B detects the newer deploy and syncs it.
    let state = read_remote_state(&client, &proj_b).await.unwrap().unwrap();
    let record_b = store_b.load_deploy_record(&proj_b.id).unwrap();
    assert!(is_newer(&state, record_b.as_ref()), "B must see A's deploy");

    let steps = drive(start_pull(
        client.clone(),
        store_b.clone(),
        proj_b.clone(),
        PullMode::Sync,
    ))
    .await;
    assert!(done_files(&steps) >= 2);
    assert_eq!(
        std::fs::read_to_string(dir_b.path().join("bot.js")).unwrap(),
        "v2"
    );
    assert_eq!(
        std::fs::read_to_string(dir_b.path().join("new.txt")).unwrap(),
        "added"
    );
    // Afterwards B is up to date again and its tree is clean.
    let state = read_remote_state(&client, &proj_b).await.unwrap().unwrap();
    let record_b = store_b.load_deploy_record(&proj_b.id).unwrap();
    assert!(!is_newer(&state, record_b.as_ref()));
    assert!(!git::status(dir_b.path()).unwrap().dirty);
}

// A new deploy that leaves a file untouched must not clobber un-deployed local
// edits to that file: the content-aware guard reports a conflict, so the app
// won't auto-pull.
#[tokio::test]
async fn sync_guard_flags_undeployed_local_edits() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    // Device A deploys v1 (bot.js + config.yml).
    let dir_a = tempfile::tempdir().unwrap();
    write_file(dir_a.path(), "bot.js", "v1");
    write_file(dir_a.path(), "config.yml", "a: 1");
    let store_a = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_a = project("aaaaaaaa-0000-0000-0000-0000000000aa", dir_a.path());
    drive(start_deploy(
        client.clone(),
        store_a.clone(),
        proj_a.clone(),
    ))
    .await;

    // Device B imports v1, then edits bot.js locally without deploying.
    let dir_b = tempfile::tempdir().unwrap();
    let store_b = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_b = project("bbbbbbbb-0000-0000-0000-0000000000bb", dir_b.path());
    drive(start_pull(
        client.clone(),
        store_b.clone(),
        proj_b.clone(),
        PullMode::InitialImport,
    ))
    .await;
    write_file(dir_b.path(), "bot.js", "local work in progress");

    // A deploys v2 that changes config.yml only (not bot.js).
    write_file(dir_a.path(), "config.yml", "a: 2");
    drive(start_deploy(client.clone(), store_a, proj_a)).await;

    let remote = read_remote_state(&client, &proj_b).await.unwrap().unwrap();
    let record = store_b.load_deploy_record(&proj_b.id).unwrap().unwrap();
    let local = feather_core::snapshot::manifest_of(dir_b.path()).unwrap();
    assert!(is_newer(&remote, Some(&record)), "B must see A's v2");
    assert!(
        feather_core::sync::sync_conflict(&record.content, &remote.content, &local),
        "B's un-deployed bot.js edit must block auto-sync (v2 didn't change bot.js)"
    );
}

// A clean teammate — or one whose local edits are all on files the deploy also
// changes — has no conflict, so the new deploy auto-syncs.
#[tokio::test]
async fn sync_guard_allows_a_clean_teammate() {
    let panel = MockPanel::spawn().await;
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    let dir_a = tempfile::tempdir().unwrap();
    write_file(dir_a.path(), "bot.js", "v1");
    let store_a = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_a = project("aaaaaaaa-0000-0000-0000-0000000000ac", dir_a.path());
    drive(start_deploy(
        client.clone(),
        store_a.clone(),
        proj_a.clone(),
    ))
    .await;

    let dir_b = tempfile::tempdir().unwrap();
    let store_b = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let proj_b = project("bbbbbbbb-0000-0000-0000-0000000000bc", dir_b.path());
    drive(start_pull(
        client.clone(),
        store_b.clone(),
        proj_b.clone(),
        PullMode::InitialImport,
    ))
    .await;

    // A deploys v2 (bot.js changes); B has made no local edits.
    write_file(dir_a.path(), "bot.js", "v2");
    drive(start_deploy(client.clone(), store_a, proj_a)).await;

    let remote = read_remote_state(&client, &proj_b).await.unwrap().unwrap();
    let record = store_b.load_deploy_record(&proj_b.id).unwrap().unwrap();
    let local = feather_core::snapshot::manifest_of(dir_b.path()).unwrap();
    assert!(is_newer(&remote, Some(&record)));
    assert!(
        !feather_core::sync::sync_conflict(&record.content, &remote.content, &local),
        "a clean teammate has no conflict — auto-sync should run"
    );

    // And the actual pull brings B to v2.
    drive(start_pull(client.clone(), store_b, proj_b, PullMode::Sync)).await;
    assert_eq!(
        std::fs::read_to_string(dir_b.path().join("bot.js")).unwrap(),
        "v2"
    );
}
