<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    listServers,
    onServerEvent,
    sendConsoleCommand,
    serverResources,
    setPower,
    subscribeServer,
    unsubscribeServer,
  } from "../api";
  import type { LiveState, PowerSignal, Server, ServerEvent } from "../types";
  import ConsoleView from "./ConsoleView.svelte";
  import ServerCard from "./ServerCard.svelte";

  const CONSOLE_BUFFER_LINES = 500;

  let servers = $state<Server[]>([]);
  let live = $state<Record<string, LiveState>>({});
  let consoles = $state<Record<string, string[]>>({});
  let openConsole = $state<string | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  const openServer = $derived(
    servers.find((server) => server.identifier === openConsole) ?? null,
  );

  function currentLive(id: string): LiveState {
    return live[id] ?? { state: null, stats: null, connected: false };
  }

  function appendConsole(id: string, line: string) {
    const lines = consoles[id] ?? [];
    lines.push(line);
    if (lines.length > CONSOLE_BUFFER_LINES) {
      lines.splice(0, lines.length - CONSOLE_BUFFER_LINES);
    }
    consoles[id] = lines;
  }

  function handleEvent(id: string, event: ServerEvent) {
    const current = currentLive(id);
    switch (event.type) {
      case "connected":
        live[id] = { ...current, connected: true };
        break;
      case "status":
        live[id] = { ...current, state: event.data };
        break;
      case "stats":
        live[id] = { connected: current.connected, state: event.data.state, stats: event.data };
        break;
      case "console":
        appendConsole(id, event.data);
        break;
      case "disconnected":
        live[id] = { ...current, connected: false };
        appendConsole(id, `[wingman] connection lost: ${event.data.reason}`);
        break;
    }
  }

  /** First paint from REST; the websocket overrides it moments later. */
  async function prefill(id: string) {
    try {
      const res = await serverResources(id);
      if (live[id]?.stats) return; // Websocket was faster.
      live[id] = {
        connected: live[id]?.connected ?? false,
        state: res.current_state,
        stats: {
          memory_bytes: res.resources.memory_bytes,
          memory_limit_bytes: 0,
          cpu_absolute: res.resources.cpu_absolute,
          disk_bytes: res.resources.disk_bytes,
          uptime: res.resources.uptime,
          state: res.current_state,
          network: {
            rx_bytes: res.resources.network_rx_bytes,
            tx_bytes: res.resources.network_tx_bytes,
          },
        },
      };
    } catch {
      // Not fatal — the websocket will deliver the state.
    }
  }

  onMount(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      try {
        servers = await listServers();
        for (const server of servers) {
          if (cancelled) break;
          const id = server.identifier;
          void prefill(id);
          // Listen before subscribing so the initial burst is not missed.
          unlisteners.push(await onServerEvent(id, (event) => handleEvent(id, event)));
          await subscribeServer(id);
        }
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) unlisten();
      for (const server of servers) void unsubscribeServer(server.identifier);
    };
  });

  async function power(id: string, signal: PowerSignal) {
    try {
      await setPower(id, signal);
    } catch (e) {
      appendConsole(id, `[wingman] power "${signal}" failed: ${e}`);
    }
  }
</script>

{#if loading}
  <p class="muted center">Loading servers…</p>
{:else if error}
  <p class="error center">{error}</p>
{:else if servers.length === 0}
  <p class="muted center">This API key has no servers.</p>
{:else}
  <div class="grid">
    {#each servers as server (server.identifier)}
      <ServerCard
        {server}
        live={currentLive(server.identifier)}
        onPower={(signal) => power(server.identifier, signal)}
        onOpenConsole={() => (openConsole = server.identifier)}
      />
    {/each}
  </div>
{/if}

{#if openServer}
  {@const id = openServer.identifier}
  <ConsoleView
    server={openServer}
    live={currentLive(id)}
    lines={consoles[id] ?? []}
    onSend={(command) => sendConsoleCommand(id, command)}
    onClose={() => (openConsole = null)}
  />
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
  }
</style>
