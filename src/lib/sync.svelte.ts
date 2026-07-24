// App-wide background sync. Whenever the app is open, Feather watches every
// project bound to a local folder on this device and, when a teammate ships a
// newer deploy, downloads it automatically — unless doing so would overwrite
// un-deployed local work on a file the deploy doesn't change (see the core's
// `sync_conflict`). Then it holds back and surfaces a banner instead.

import { checkRemoteDeploy, pullProject } from "./api";
import type { CloudProject } from "./cloud";
import type { ProjectConfig } from "./types";

export type SyncStatus = "idle" | "syncing" | "conflict";

// Per-project status plus a version counter bumped after each successful sync,
// so an open Deploy tab can reload when its project is updated underneath it.
export const syncState = $state<Record<string, { status: SyncStatus; version: number }>>({});

// Projects with a user-initiated engine op in flight (deploy/import/rollback).
// The background sweep skips these so it never races the user.
const busy = new Set<string>();

/** Build the engine ProjectConfig for a cloud project bound to a local folder. */
export function projectConfig(project: CloudProject, localPath: string): ProjectConfig {
  return {
    id: project.id,
    name: project.name,
    local_path: localPath,
    panel_id: project.panel_id ?? "",
    server_identifier: project.server_identifier ?? "",
    target_dir: project.target_dir,
    build_command: project.build_command,
    post_deploy: project.post_deploy,
    auto_backup: project.auto_backup,
  };
}

export function syncStatus(projectId: string): SyncStatus {
  return syncState[projectId]?.status ?? "idle";
}

export function syncVersion(projectId: string): number {
  return syncState[projectId]?.version ?? 0;
}

function patch(id: string, next: Partial<{ status: SyncStatus; version: number }>) {
  const cur = syncState[id] ?? { status: "idle" as SyncStatus, version: 0 };
  syncState[id] = { ...cur, ...next };
}

/** DeployPanel marks its project busy around its own engine ops so the
 *  background sweep never runs a pull at the same time. */
export function setSyncBusy(projectId: string, isBusy: boolean): void {
  if (isBusy) busy.add(projectId);
  else busy.delete(projectId);
}

/**
 * Check one project for a newer team deploy and, when it's safe, pull it in.
 * Silent and best-effort: network errors just retry on the next sweep.
 */
export async function checkProject(config: ProjectConfig): Promise<void> {
  const id = config.id;
  if (busy.has(id) || syncStatus(id) === "syncing") return;
  let info;
  try {
    info = await checkRemoteDeploy(config);
  } catch {
    return; // offline / panel unreachable — try again next sweep
  }
  if (!info.newer) {
    if (syncStatus(id) !== "idle") patch(id, { status: "idle" });
    return;
  }
  if (info.conflict) {
    if (syncStatus(id) !== "conflict") patch(id, { status: "conflict" });
    return;
  }
  // A newer deploy that's safe to apply: download it now.
  if (busy.has(id)) return;
  patch(id, { status: "syncing" });
  try {
    await pullProject(config, "sync");
    patch(id, { status: "idle", version: syncVersion(id) + 1 });
  } catch {
    patch(id, { status: "idle" });
  }
}
