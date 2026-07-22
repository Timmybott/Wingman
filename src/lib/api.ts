// Typed wrappers around the Tauri IPC commands (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CommitInfo,
  DeployStatus,
  DeployStep,
  FileEntry,
  PowerSignal,
  ProjectConfig,
  RemoteDeployInfo,
  RepoStatus,
  Server,
  ServerEvent,
  ServerStats,
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

export function listProjects(): Promise<ProjectConfig[]> {
  return invoke<ProjectConfig[]>("list_projects");
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

/** Create (empty id) or update a project. */
export function saveProject(project: ProjectConfig): Promise<ProjectConfig> {
  return invoke<ProjectConfig>("save_project", { project });
}

export function deleteProject(projectId: string): Promise<void> {
  return invoke<void>("delete_project", { projectId });
}

/** Start a deploy; progress arrives via onDeployEvent. */
export function deployProject(projectId: string): Promise<void> {
  return invoke<void>("deploy_project", { projectId });
}

export function onDeployEvent(
  projectId: string,
  handler: (step: DeployStep) => void,
): Promise<UnlistenFn> {
  return listen<DeployStep>(`deploy-event-${projectId}`, (e) => handler(e.payload));
}

/** Deploy an old commit; progress arrives on the same deploy-event channel. */
export function rollbackProject(projectId: string, commitId: string): Promise<void> {
  return invoke<void>("rollback_project", { projectId, commitId });
}

/**
 * Pull server files into the local folder. "import" fills an empty folder
 * right after linking; "sync" updates a clean working tree when another
 * device deployed. Progress arrives on the deploy-event channel.
 */
export function pullProject(projectId: string, mode: "import" | "sync"): Promise<void> {
  return invoke<void>("pull_project", { projectId, mode });
}

export function checkRemoteDeploy(projectId: string): Promise<RemoteDeployInfo> {
  return invoke<RemoteDeployInfo>("check_remote_deploy", { projectId });
}

export function repoStatus(projectId: string): Promise<RepoStatus> {
  return invoke<RepoStatus>("repo_status", { projectId });
}

export function commitProject(projectId: string, message: string): Promise<CommitInfo> {
  return invoke<CommitInfo>("commit_project", { projectId, message });
}

export function projectHistory(projectId: string, limit?: number): Promise<CommitInfo[]> {
  return invoke<CommitInfo[]>("project_history", { projectId, limit });
}

export function deployStatus(projectId: string): Promise<DeployStatus> {
  return invoke<DeployStatus>("deploy_status", { projectId });
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
