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
