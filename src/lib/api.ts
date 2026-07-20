// Typed wrappers around the Tauri IPC commands (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CommitInfo,
  DeployStatus,
  DeployStep,
  FileEntry,
  PanelConfig,
  PowerSignal,
  ProjectConfig,
  RemoteDeployInfo,
  RepoStatus,
  Server,
  ServerEvent,
  ServerStats,
} from "./types";

export function getPanel(): Promise<PanelConfig | null> {
  return invoke<PanelConfig | null>("get_panel");
}

/** Dry-run credentials check; resolves to the number of visible servers. */
export function testConnection(baseUrl: string, apiKey: string): Promise<number> {
  return invoke<number>("test_connection", { baseUrl, apiKey });
}

export function savePanel(
  name: string,
  baseUrl: string,
  apiKey: string,
): Promise<PanelConfig> {
  return invoke<PanelConfig>("save_panel", { name, baseUrl, apiKey });
}

export function removePanel(): Promise<void> {
  return invoke<void>("remove_panel");
}

export function listServers(): Promise<Server[]> {
  return invoke<Server[]>("list_servers");
}

export function serverResources(identifier: string): Promise<ServerStats> {
  return invoke<ServerStats>("server_resources", { identifier });
}

export function setPower(identifier: string, signal: PowerSignal): Promise<void> {
  return invoke<void>("set_power", { identifier, signal });
}

/** Open the server's websocket in the Rust core (idempotent). */
export function subscribeServer(identifier: string): Promise<void> {
  return invoke<void>("subscribe_server", { identifier });
}

export function unsubscribeServer(identifier: string): Promise<void> {
  return invoke<void>("unsubscribe_server", { identifier });
}

export function sendConsoleCommand(identifier: string, command: string): Promise<void> {
  return invoke<void>("send_console_command", { identifier, command });
}

/**
 * Listen to the live events of one server. Register BEFORE calling
 * subscribeServer so the initial Connected/Status burst is not missed.
 */
export function onServerEvent(
  identifier: string,
  handler: (event: ServerEvent) => void,
): Promise<UnlistenFn> {
  return listen<ServerEvent>(`server-event-${identifier}`, (e) => handler(e.payload));
}

export function listProjects(): Promise<ProjectConfig[]> {
  return invoke<ProjectConfig[]>("list_projects");
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
  identifier: string,
  directory: string,
): Promise<FileEntry[]> {
  return invoke<FileEntry[]>("list_server_files", { identifier, directory });
}

export function deleteServerFiles(
  identifier: string,
  root: string,
  files: string[],
): Promise<void> {
  return invoke<void>("delete_server_files", { identifier, root, files });
}

export function createServerFolder(
  identifier: string,
  root: string,
  name: string,
): Promise<void> {
  return invoke<void>("create_server_folder", { identifier, root, name });
}
