//! Integration tests: the deploy engine against the mock panel's virtual
//! filesystem.

use feather_core::deploy::{start_deploy, start_rollback, DeployStep, BACKUP_PREFIX};
use feather_core::models::PowerState;
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

/// A small realistic project: app files plus ignored build artifacts. The
/// engine turns the folder into a real git repository on the first deploy.
fn sample_project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_file(dir.path(), "index.js", "console.log('hi')");
    write_file(dir.path(), "config/settings.yml", "motd: hello");
    write_file(dir.path(), "node_modules/lib/x.js", "x");
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

/// Drive an engine handle to its terminal event, collecting every step.
async fn drive(mut handle: feather_core::DeployHandle) -> Vec<DeployStep> {
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

async fn run_deploy(
    panel: &MockPanel,
    store: &ConfigStore,
    project: &ProjectConfig,
) -> Vec<DeployStep> {
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    drive(start_deploy(client, store.clone(), project.clone())).await
}

async fn run_rollback(
    panel: &MockPanel,
    store: &ConfigStore,
    project: &ProjectConfig,
    commit: &str,
) -> Vec<DeployStep> {
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    drive(start_rollback(
        client,
        store.clone(),
        project.clone(),
        commit.to_string(),
    ))
    .await
}

fn assert_done(steps: &[DeployStep]) -> (usize, usize) {
    match steps.last() {
        Some(DeployStep::Done { files, deleted }) => (*files, *deleted),
        other => panic!("expected Done as final step, got {other:?}"),
    }
}

/// Server files without the multi-device sync marker every deploy writes.
fn visible_files(panel: &MockPanel, server: &str) -> Vec<String> {
    panel
        .server_files(server)
        .into_iter()
        .filter(|f| !f.ends_with(feather_core::sync::STATE_FILE))
        .collect()
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
        visible_files(&panel, SERVER),
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
        visible_files(&panel, SERVER),
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
        visible_files(&panel, SERVER),
        vec!["config/settings.yml", "index.js"]
    );

    std::fs::remove_file(dir.path().join("config/settings.yml")).unwrap();
    let steps = run_deploy(&panel, &store, &project).await;
    let (files, deleted) = assert_done(&steps);
    assert_eq!(files, 1);
    assert_eq!(deleted, 1, "settings.yml was in the last manifest");

    assert_eq!(
        visible_files(&panel, SERVER),
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

    let kinds = step_kinds(&steps);
    // Uploading may appear several times (progress); dedup for the order check.
    let mut order = kinds.clone();
    order.dedup();
    assert_eq!(
        order,
        vec![
            "committing",
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

fn step_kinds(steps: &[DeployStep]) -> Vec<&'static str> {
    steps
        .iter()
        .map(|s| match s {
            DeployStep::Committing => "committing",
            DeployStep::CheckingOut => "checking_out",
            DeployStep::Building => "building",
            DeployStep::BuildOutput { .. } => "build_output",
            DeployStep::BackingUp => "backing_up",
            DeployStep::BackupSkipped { .. } => "backup_skipped",
            DeployStep::Scanning => "scanning",
            DeployStep::Packing { .. } => "packing",
            DeployStep::Uploading { .. } => "uploading",
            DeployStep::Downloading { .. } => "downloading",
            DeployStep::Importing => "importing",
            DeployStep::Extracting => "extracting",
            DeployStep::CleaningUp => "cleaning_up",
            DeployStep::Restarting => "restarting",
            DeployStep::Done { .. } => "done",
            DeployStep::Failed { .. } => "failed",
        })
        .collect()
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
async fn records_the_deployed_commit() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let project = project(dir.path(), "", PostDeployAction::Notify);

    assert_done(&run_deploy(&panel, &store, &project).await);

    let record = store.load_deploy_record(&project.id).unwrap().unwrap();
    let commit = record.commit.expect("deploy must record a commit");
    let history = git::log(dir.path(), 10).unwrap();
    assert_eq!(history.len(), 1, "auto-commit created exactly one commit");
    assert_eq!(history[0].id, commit);
    assert!(history[0].summary.starts_with("Deploy at "));
    // A clean second deploy records the same commit without creating a new one.
    assert_done(&run_deploy(&panel, &store, &project).await);
    assert_eq!(git::log(dir.path(), 10).unwrap().len(), 1);
}

#[tokio::test]
async fn build_command_streams_output_and_ships_artifacts() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let mut project = project(dir.path(), "", PostDeployAction::Notify);
    project.build_command = Some("echo hello-from-build && printf artifact > build.txt".into());

    let steps = run_deploy(&panel, &store, &project).await;
    assert_done(&steps);
    assert!(
        steps.iter().any(|s| matches!(
            s,
            DeployStep::BuildOutput { line } if line.contains("hello-from-build")
        )),
        "expected the build output line, got {steps:?}"
    );
    assert_eq!(
        panel.file_contents(SERVER, "build.txt").unwrap(),
        b"artifact",
        "the build artifact is part of the deploy"
    );
}

#[tokio::test]
async fn failing_build_aborts_before_upload() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let mut project = project(dir.path(), "", PostDeployAction::Notify);
    project.build_command = Some("echo boom && exit 3".into());

    let steps = run_deploy(&panel, &store, &project).await;
    match steps.last() {
        Some(DeployStep::Failed { message }) => {
            assert!(
                message.contains("build command failed"),
                "message: {message}"
            )
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    assert!(
        panel.server_files(SERVER).is_empty(),
        "nothing may reach the server when the build fails"
    );
}

#[tokio::test]
async fn creates_and_completes_a_pre_deploy_backup() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let mut project = project(dir.path(), "", PostDeployAction::Notify);
    project.auto_backup = true;

    let steps = run_deploy(&panel, &store, &project).await;
    assert_done(&steps);
    assert!(steps.iter().any(|s| matches!(s, DeployStep::BackingUp)));

    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    let backups = client.list_backups(SERVER).await.unwrap();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].name.starts_with(BACKUP_PREFIX));
    assert!(
        backups[0].completed_at.is_some(),
        "engine waits for completion"
    );
    assert!(backups[0].is_successful);
}

#[tokio::test]
async fn rotates_own_backups_when_the_limit_is_reached() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    // Fill all 3 slots of a1b2c3d4 with Feather-created backups.
    let oldest = client
        .create_backup(SERVER, &format!("{BACKUP_PREFIX}old-1"))
        .await
        .unwrap();
    client
        .create_backup(SERVER, &format!("{BACKUP_PREFIX}old-2"))
        .await
        .unwrap();
    client
        .create_backup(SERVER, &format!("{BACKUP_PREFIX}old-3"))
        .await
        .unwrap();

    let mut project = project(dir.path(), "", PostDeployAction::Notify);
    project.auto_backup = true;
    assert_done(&run_deploy(&panel, &store, &project).await);

    let backups = client.list_backups(SERVER).await.unwrap();
    assert_eq!(backups.len(), 3, "limit stays respected");
    assert!(
        !backups.iter().any(|b| b.uuid == oldest.uuid),
        "the oldest Feather backup was rotated out"
    );
}

#[tokio::test]
async fn skips_backup_instead_of_touching_foreign_backups() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    // b2c3d4e5 has a single slot, filled with a user-created backup.
    let foreign = client
        .create_backup("b2c3d4e5", "my-precious-backup")
        .await
        .unwrap();

    let mut project = project(dir.path(), "", PostDeployAction::Notify);
    project.server_identifier = "b2c3d4e5".into();
    project.auto_backup = true;
    let steps = run_deploy(&panel, &store, &project).await;
    assert_done(&steps);
    assert!(
        steps
            .iter()
            .any(|s| matches!(s, DeployStep::BackupSkipped { .. })),
        "expected BackupSkipped, got {steps:?}"
    );

    let backups = client.list_backups("b2c3d4e5").await.unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].uuid, foreign.uuid, "foreign backup untouched");
}

#[tokio::test]
async fn rollback_restores_an_old_deploy() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let project = project(dir.path(), "", PostDeployAction::Notify);

    // v1
    assert_done(&run_deploy(&panel, &store, &project).await);
    let v1_commit = store
        .load_deploy_record(&project.id)
        .unwrap()
        .unwrap()
        .commit
        .unwrap();

    // v2: change a file, add a new one.
    write_file(dir.path(), "index.js", "console.log('v2')");
    write_file(dir.path(), "extra.txt", "only in v2");
    assert_done(&run_deploy(&panel, &store, &project).await);
    assert_eq!(
        panel.file_contents(SERVER, "index.js").unwrap(),
        b"console.log('v2')"
    );
    assert!(panel.file_contents(SERVER, "extra.txt").is_some());

    // Rollback to v1: old content restored, v2-only file removed remotely,
    // and the local working tree keeps its v2 state.
    let steps = run_rollback(&panel, &store, &project, &v1_commit).await;
    assert!(steps.iter().any(|s| matches!(s, DeployStep::CheckingOut)));
    assert_done(&steps);

    assert_eq!(
        panel.file_contents(SERVER, "index.js").unwrap(),
        b"console.log('hi')"
    );
    assert!(
        panel.file_contents(SERVER, "extra.txt").is_none(),
        "the v2-only file is deleted by the manifest diff"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("index.js")).unwrap(),
        "console.log('v2')",
        "the working tree is untouched by the rollback"
    );
    let record = store.load_deploy_record(&project.id).unwrap().unwrap();
    assert_eq!(record.commit.as_deref(), Some(v1_commit.as_str()));
}

#[tokio::test]
async fn file_listing_reflects_a_deploy() {
    let panel = MockPanel::spawn().await;
    let dir = sample_project_dir();
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();

    assert_done(
        &run_deploy(
            &panel,
            &store,
            &project(dir.path(), "", PostDeployAction::Notify),
        )
        .await,
    );

    let root = client.list_files(SERVER, "/").await.unwrap();
    let names: Vec<(&str, bool)> = root
        .iter()
        .filter(|e| e.name != feather_core::sync::STATE_FILE)
        .map(|e| (e.name.as_str(), e.is_file))
        .collect();
    assert_eq!(names, vec![("config", false), ("index.js", true)]);
    assert!(root.iter().any(|e| e.is_file && e.size > 0));
    // The sync marker itself is part of the listing.
    assert!(root
        .iter()
        .any(|e| e.name == feather_core::sync::STATE_FILE));

    let sub = client.list_files(SERVER, "/config").await.unwrap();
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0].name, "settings.yml");
    assert!(sub[0].is_file);

    // Folders created via the API show up even while empty.
    client.create_folder(SERVER, "/", "plugins").await.unwrap();
    let root = client.list_files(SERVER, "/").await.unwrap();
    assert!(root.iter().any(|e| e.name == "plugins" && !e.is_file));
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

// --- Bundle deploy (delta model) -------------------------------------------

use feather_core::snapshot::{delta_zip, CommitDelta, Manifest};
use feather_core::start_apply_bundle;

/// Build a commit delta for a working tree containing `files`, diffed against
/// `base`. Returns the delta and the resulting manifest (the new committed
/// state), so the next commit can be chained on top.
fn commit_delta(base: &Manifest, files: &[(&str, &str)]) -> (CommitDelta, Manifest) {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        write_file(dir.path(), rel, content);
    }
    let (zip, resulting, deleted) = delta_zip(dir.path(), base).unwrap();
    (CommitDelta { zip, deleted }, resulting)
}

async fn apply_bundle(
    panel: &MockPanel,
    store: &ConfigStore,
    project: &ProjectConfig,
    base: Manifest,
    deltas: Vec<CommitDelta>,
) -> Vec<DeployStep> {
    let client = PanelClient::new(&panel.base_url(), API_KEY).unwrap();
    drive(start_apply_bundle(
        client,
        store.clone(),
        project.clone(),
        base,
        deltas,
        Some("newest-commit".into()),
    ))
    .await
}

#[tokio::test]
async fn bundle_deploy_applies_only_its_commits_and_combines_them() {
    let panel = MockPanel::spawn().await;
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    // apply_bundle never reads the local folder — a throwaway path is fine.
    let project = project(
        tempfile::tempdir().unwrap().path(),
        "",
        PostDeployAction::Notify,
    );

    // Two commits by (conceptually) different members touching different files,
    // chained: commit B is built on top of A's committed state.
    let base = Manifest::new(); // empty server to start
    let (a, m_a) = commit_delta(&base, &[("shared.txt", "s"), ("x.txt", "xx")]);
    let (b, m_b) = commit_delta(
        &m_a,
        &[("shared.txt", "s"), ("x.txt", "xx"), ("y.txt", "yy")],
    );

    let steps = apply_bundle(&panel, &store, &project, base, vec![a, b]).await;
    let (files, deleted) = assert_done(&steps);
    assert_eq!(files, 3, "resulting server state has shared, x and y");
    assert_eq!(deleted, 0);

    // Both members' changes landed, even though a deploy ships no local folder.
    assert_eq!(
        visible_files(&panel, SERVER),
        vec!["shared.txt", "x.txt", "y.txt"]
    );
    assert_eq!(panel.file_contents(SERVER, "x.txt").unwrap(), b"xx");
    assert_eq!(panel.file_contents(SERVER, "y.txt").unwrap(), b"yy");

    // A second bundle that modifies shared.txt and deletes y.txt: the deploy is
    // purely the sum of its commit, so y.txt goes and shared.txt updates.
    let (c, _m_c) = commit_delta(&m_b, &[("shared.txt", "s2"), ("x.txt", "xx")]);
    let steps = apply_bundle(&panel, &store, &project, m_b, vec![c]).await;
    let (files, deleted) = assert_done(&steps);
    assert_eq!(files, 2, "shared and x remain");
    assert_eq!(deleted, 1, "y.txt was removed");

    assert_eq!(visible_files(&panel, SERVER), vec!["shared.txt", "x.txt"]);
    assert_eq!(panel.file_contents(SERVER, "shared.txt").unwrap(), b"s2");
    assert!(panel.file_contents(SERVER, "y.txt").is_none());
}

#[tokio::test]
async fn bundle_deploy_without_commits_fails() {
    let panel = MockPanel::spawn().await;
    let store = ConfigStore::new(tempfile::tempdir().unwrap().path());
    let project = project(
        tempfile::tempdir().unwrap().path(),
        "",
        PostDeployAction::Notify,
    );

    let steps = apply_bundle(&panel, &store, &project, Manifest::new(), vec![]).await;
    match steps.last() {
        Some(DeployStep::Failed { message }) => {
            assert!(message.contains("nothing to deploy"), "message: {message}")
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}
