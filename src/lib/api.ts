// Typed wrappers around the Tauri IPC commands (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CommitInfo,
  DeployStatus,
  DeployStep,
  Diff,
  FileEntry,
  Manifest,
  PowerSignal,
  ProjectConfig,
  RemoteDeployInfo,
  RepoStatus,
  Server,
  ServerEvent,
  ServerStats,
  SnapshotUpload,
} from "./types";

/** Dry-run credentials check; resolves to the number of visible servers. */
export function testConnection(baseUrl: string, apiKey: string): Promise<number> {
  return invoke<number>("test_connection", { baseUrl, apiKey });
}

/**
 * Connect a team panel for this session, keyed by its cloud panel id. The
 * decrypted key (fetched via panelApiKey) is held in memory by the Rust core,
 * never on disk. Several panels can be connected at once.
 */
export function setActivePanel(
  panelId: string,
  baseUrl: string,
  apiKey: string,
): Promise<void> {
  return invoke<void>("set_active_panel", { panelId, baseUrl, apiKey });
}

/** Disconnect one panel and tear down its live sockets. */
export function clearActivePanel(panelId: string): Promise<void> {
  return invoke<void>("clear_active_panel", { panelId });
}

export function listServers(panelId: string): Promise<Server[]> {
  return invoke<Server[]>("list_servers", { panelId });
}

export function serverResources(panelId: string, identifier: string): Promise<ServerStats> {
  return invoke<ServerStats>("server_resources", { panelId, identifier });
}

export function setPower(
  panelId: string,
  identifier: string,
  signal: PowerSignal,
): Promise<void> {
  return invoke<void>("set_power", { panelId, identifier, signal });
}

/** Open the server's websocket in the Rust core (idempotent). */
export function subscribeServer(panelId: string, identifier: string): Promise<void> {
  return invoke<void>("subscribe_server", { panelId, identifier });
}

export function unsubscribeServer(panelId: string, identifier: string): Promise<void> {
  return invoke<void>("unsubscribe_server", { panelId, identifier });
}

export function sendConsoleCommand(
  panelId: string,
  identifier: string,
  command: string,
): Promise<void> {
  return invoke<void>("send_console_command", { panelId, identifier, command });
}

/**
 * Listen to the live events of one server. Register BEFORE calling
 * subscribeServer so the initial Connected/Status burst is not missed.
 */
export function onServerEvent(
  panelId: string,
  identifier: string,
  handler: (event: ServerEvent) => void,
): Promise<UnlistenFn> {
  return listen<ServerEvent>(`server-event-${panelId}-${identifier}`, (e) =>
    handler(e.payload),
  );
}

// --- Per-device local folder bindings for cloud projects -------------------

/**
 * Bind a cloud project to a local folder on this device. Resolves to whether
 * the folder is currently empty (so the caller can offer to import).
 */
export function setProjectPath(projectId: string, path: string): Promise<boolean> {
  return invoke<boolean>("set_project_path", { projectId, path });
}

/** The local folder bound to a project on this device, if any. */
export function getProjectPath(projectId: string): Promise<string | null> {
  return invoke<string | null>("get_project_path", { projectId });
}

/** Remove this device's local binding for a project (leaves files untouched). */
export function removeProjectPath(projectId: string): Promise<void> {
  return invoke<void>("remove_project_path", { projectId });
}

/**
 * Remove a project from this device: drop its binding and deploy record, and
 * when `deleteFiles` is true, delete the bound folder recursively. Used for
 * "remove from Feather" (deleteFiles false) and for processing a
 * "delete everywhere" tombstone (deleteFiles true).
 */
export function removeLocalProject(projectId: string, deleteFiles: boolean): Promise<void> {
  return invoke<void>("remove_local_project", { projectId, deleteFiles });
}

/**
 * The engine takes the full project config (built from the cloud project plus
 * this device's local folder), so it needs no local project store.
 */

export function onDeployEvent(
  projectId: string,
  handler: (step: DeployStep) => void,
): Promise<UnlistenFn> {
  return listen<DeployStep>(`deploy-event-${projectId}`, (e) => handler(e.payload));
}

/** Deploy an old commit; progress arrives on the same deploy-event channel. */
export function rollbackProject(project: ProjectConfig, commitId: string): Promise<void> {
  return invoke<void>("rollback_project", { project, commitId });
}

/**
 * Roll the server back to a cloud commit: the Rust side downloads that commit's
 * snapshot from the storage backend and deploys it. Progress arrives on the
 * deploy-event channel. The local folder is not touched.
 */
export function rollbackToSnapshot(
  project: ProjectConfig,
  endpoint: string,
  token: string,
  anonKey: string,
  projectId: string,
  kind: string,
  snapshotId: string,
): Promise<void> {
  return invoke<void>("rollback_to_snapshot", {
    project,
    endpoint,
    token,
    anonKey,
    projectId,
    kind,
    snapshotId,
  });
}

/**
 * Pull server files into the local folder. "import" fills an empty folder
 * right after linking; "sync" updates a clean working tree when another
 * device deployed. Progress arrives on the deploy-event channel.
 */
export function pullProject(project: ProjectConfig, mode: "import" | "sync"): Promise<void> {
  return invoke<void>("pull_project", { project, mode });
}

export function checkRemoteDeploy(project: ProjectConfig): Promise<RemoteDeployInfo> {
  return invoke<RemoteDeployInfo>("check_remote_deploy", { project });
}

/** Every project → local-folder binding on this device (drives background sync). */
export function listProjectPaths(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("list_project_paths");
}

export function repoStatus(project: ProjectConfig): Promise<RepoStatus> {
  return invoke<RepoStatus>("repo_status", { project });
}

export function commitProject(project: ProjectConfig, message: string): Promise<CommitInfo> {
  return invoke<CommitInfo>("commit_project", { project, message });
}

export function projectHistory(project: ProjectConfig, limit?: number): Promise<CommitInfo[]> {
  return invoke<CommitInfo[]>("project_history", { project, limit });
}

// --- Cloud commits (M22): manifest, diff and snapshot upload ---------------

/** The content manifest (path → hash) of the project's local folder. */
export function projectManifest(project: ProjectConfig): Promise<Manifest> {
  return invoke<Manifest>("project_manifest", { project });
}

/** Diff the local folder against a base manifest (e.g. the server state). */
export function projectDiff(project: ProjectConfig, base: Manifest): Promise<Diff> {
  return invoke<Diff>("project_diff", { project, base });
}

/**
 * Upload this commit's delta: only what changed relative to `base` (the
 * accumulated committed state). Returns the number of changed paths and the
 * full resulting manifest, recorded as the commit's state so a deploy can apply
 * the whole bundle.
 */
export function uploadCommitDelta(
  project: ProjectConfig,
  base: Manifest,
  endpoint: string,
  token: string,
  anonKey: string,
  projectId: string,
  commitId: string,
): Promise<SnapshotUpload> {
  return invoke<SnapshotUpload>("upload_commit_delta", {
    project,
    base,
    endpoint,
    token,
    anonKey,
    projectId,
    commitId,
  });
}

/** One commit of the current Deploy bundle, passed to deployBundle oldest first. */
export interface BundleCommitArg {
  id: string;
  manifest: Manifest;
}

/**
 * Deploy the current bundle: the Rust side downloads each commit's delta and
 * applies them over `base` (the current server state) — nothing uncommitted is
 * shipped. Progress arrives on the deploy-event channel.
 */
export function deployBundle(
  project: ProjectConfig,
  endpoint: string,
  token: string,
  anonKey: string,
  projectId: string,
  bundleId: string,
  base: Manifest,
  commits: BundleCommitArg[],
): Promise<void> {
  return invoke<void>("deploy_bundle", {
    project,
    endpoint,
    token,
    anonKey,
    projectId,
    bundleId,
    base,
    commits,
  });
}

export function deployStatus(project: ProjectConfig): Promise<DeployStatus> {
  return invoke<DeployStatus>("deploy_status", { project });
}

export function listServerFiles(
  panelId: string,
  identifier: string,
  directory: string,
): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_server_files", { panelId, identifier, directory });
}

export function deleteServerFiles(
  panelId: string,
  identifier: string,
  root: string,
  files: string[],
): Promise<void> {
  return invoke<void>("delete_server_files", { panelId, identifier, root, files });
}

export function createServerFolder(
  panelId: string,
  identifier: string,
  root: string,
  name: string,
): Promise<void> {
  return invoke<void>("create_server_folder", { panelId, identifier, root, name });
}

/** Read a server file as UTF-8 text (rejects non-text files). */
export function readServerFile(
  panelId: string,
  identifier: string,
  path: string,
): Promise<string> {
  return invoke<string>("read_server_file", { panelId, identifier, path });
}

/** Overwrite a server file with new text content. */
export function writeServerFile(
  panelId: string,
  identifier: string,
  path: string,
  content: string,
): Promise<void> {
  return invoke<void>("write_server_file", { panelId, identifier, path, content });
}

/** Read a file inside the project's local folder as UTF-8 text. */
export function readLocalFile(project: ProjectConfig, path: string): Promise<string> {
  return invoke<string>("read_local_file", { project, path });
}

/**
 * One file's text from a commit's stored delta. `found` is false when the path
 * is not in that commit's zip (inherited or removed) — walk back to the commit
 * that actually wrote it.
 */
export interface SnapshotFile {
  found: boolean;
  text: string;
}

export function snapshotFile(
  endpoint: string,
  token: string,
  anonKey: string,
  projectId: string,
  commitId: string,
  path: string,
): Promise<SnapshotFile> {
  return invoke<SnapshotFile>("snapshot_file", {
    endpoint,
    token,
    anonKey,
    projectId,
    commitId,
    path,
  });
}
