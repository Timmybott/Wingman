// Typed wrappers around the Tauri IPC commands (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";
import type { PanelConfig, Server, ServerStats } from "./types";

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
