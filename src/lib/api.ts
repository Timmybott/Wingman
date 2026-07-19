// Typed wrappers around the Tauri IPC commands (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PanelConfig, PowerSignal, Server, ServerEvent, ServerStats } from "./types";

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
