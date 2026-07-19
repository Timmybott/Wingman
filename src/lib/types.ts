// Mirrors the serde models in crates/wingman-core/src/{models,config}.rs.

export interface PanelConfig {
  id: string;
  name: string;
  base_url: string;
}

export interface ServerLimits {
  /** MiB, 0 = unlimited */
  memory: number;
  swap: number;
  /** MiB, 0 = unlimited */
  disk: number;
  io: number | null;
  /** percent across cores, 0 = unlimited */
  cpu: number;
}

export interface FeatureLimits {
  databases: number;
  allocations: number;
  backups: number;
}

export interface Server {
  identifier: string;
  uuid: string;
  name: string;
  description: string;
  node: string;
  limits: ServerLimits;
  feature_limits: FeatureLimits;
  status: string | null;
  is_suspended: boolean;
  is_installing: boolean;
}

export type PowerState = "running" | "starting" | "stopping" | "offline";

export interface ResourceUsage {
  memory_bytes: number;
  cpu_absolute: number;
  disk_bytes: number;
  network_rx_bytes: number;
  network_tx_bytes: number;
  /** milliseconds */
  uptime: number;
}

export interface ServerStats {
  current_state: PowerState;
  is_suspended: boolean;
  resources: ResourceUsage;
}

export type PowerSignal = "start" | "stop" | "restart" | "kill";

export type PostDeployAction = "restart" | "notify";

/** Mirrors wingman_core::config::ProjectConfig. */
export interface ProjectConfig {
  id: string;
  name: string;
  local_path: string;
  panel_id: string;
  server_identifier: string;
  target_dir: string;
  build_command: string | null;
  post_deploy: PostDeployAction;
  auto_backup: boolean;
}

/** Mirrors wingman_core::deploy::DeployStep (serde tag "step"). */
export type DeployStep =
  | { step: "committing" }
  | { step: "checking_out" }
  | { step: "building" }
  | { step: "build_output"; line: string }
  | { step: "backing_up" }
  | { step: "backup_skipped"; reason: string }
  | { step: "scanning" }
  | { step: "packing"; files: number }
  | { step: "uploading"; percent: number }
  | { step: "extracting" }
  | { step: "cleaning_up" }
  | { step: "restarting" }
  | { step: "done"; files: number; deleted: number }
  | { step: "failed"; message: string };

/** Mirrors wingman_core::git::CommitInfo. */
export interface CommitInfo {
  id: string;
  short_id: string;
  summary: string;
  author: string;
  /** unix seconds */
  timestamp: number;
}

export interface ChangedFile {
  path: string;
  kind: "new" | "modified" | "deleted" | "renamed" | "other";
}

export interface RepoStatus {
  dirty: boolean;
  changed: ChangedFile[];
  head: CommitInfo | null;
}

export interface DeployStatus {
  last_deploy: { timestamp: number; commit: string | null } | null;
  commits_since: number | null;
}

/** Live snapshot pushed by Wings over the websocket. */
export interface WsStats {
  memory_bytes: number;
  memory_limit_bytes: number;
  cpu_absolute: number;
  disk_bytes: number;
  /** milliseconds */
  uptime: number;
  state: PowerState;
  network: { rx_bytes: number; tx_bytes: number };
}

/** Aggregated live view of one server, fed by its websocket events. */
export interface LiveState {
  state: PowerState | null;
  stats: WsStats | null;
  connected: boolean;
}

/** Mirrors wingman_core::ws::ServerEvent (serde tag/content). */
export type ServerEvent =
  | { type: "connected" }
  | { type: "status"; data: PowerState }
  | { type: "stats"; data: WsStats }
  | { type: "console"; data: string }
  | { type: "disconnected"; data: { reason: string } };
