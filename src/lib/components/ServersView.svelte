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

  export type PanelInfo = { id: string; name: string };

  let {
    panels,
    onManage,
  }: {
    panels: PanelInfo[];
    onManage: () => void;
  } = $props();

  const CONSOLE_BUFFER_LINES = 500;

  type Entry = { panelId: string; panelName: string; server: Server };

  let entries = $state<Entry[]>([]);
  let panelErrors = $state<Record<string, string>>({});
  let live = $state<Record<string, LiveState>>({});
  let consoles = $state<Record<string, string[]>>({});
  let openKey = $state<string | null>(null);
  let loading = $state(true);

  let cancelled = false;
  const unlisteners: UnlistenFn[] = [];
  const subscribed: { panelId: string; id: string }[] = [];

  const keyOf = (panelId: string, id: string) => `${panelId}/${id}`;
  const openEntry = $derived(
    entries.find((e) => keyOf(e.panelId, e.server.identifier) === openKey) ?? null,
  );
  const grouped = $derived(
    panels.map((p) => ({ panel: p, servers: entries.filter((e) => e.panelId === p.id) })),
  );

  function currentLive(k: string): LiveState {
    return live[k] ?? { state: null, stats: null, connected: false };
  }

  function appendConsole(k: string, line: string) {
    const lines = consoles[k] ?? [];
    lines.push(line);
    if (lines.length > CONSOLE_BUFFER_LINES) {
      lines.splice(0, lines.length - CONSOLE_BUFFER_LINES);
    }
    consoles[k] = lines;
  }

  function handleEvent(k: string, event: ServerEvent) {
    const current = currentLive(k);
    switch (event.type) {
      case "connected":
        live[k] = { ...current, connected: true };
        break;
      case "status":
        live[k] = { ...current, state: event.data };
        break;
      case "stats":
        live[k] = { connected: current.connected, state: event.data.state, stats: event.data };
        break;
      case "console":
        appendConsole(k, event.data);
        break;
      case "disconnected":
        live[k] = { ...current, connected: false };
        appendConsole(k, `[feather] connection lost: ${event.data.reason}`);
        break;
    }
  }

  /** First paint from REST; the websocket overrides it moments later. */
  async function prefill(panelId: string, id: string) {
    const k = keyOf(panelId, id);
    try {
      const res = await serverResources(panelId, id);
      if (live[k]?.stats) return;
      live[k] = {
        connected: live[k]?.connected ?? false,
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
    (async () => {
      // Gather servers from every connected panel; one failing panel must not
      // hide the others.
      for (const panel of panels) {
        if (cancelled) break;
        try {
          const servers = await listServers(panel.id);
          for (const server of servers) {
            entries.push({ panelId: panel.id, panelName: panel.name, server });
          }
        } catch (e) {
          panelErrors[panel.id] = String(e);
        }
      }
      loading = false;
      for (const entry of [...entries]) {
        if (cancelled) break;
        const { panelId, server } = entry;
        const id = server.identifier;
        void prefill(panelId, id);
        // Listen before subscribing so the initial burst is not missed.
        unlisteners.push(
          await onServerEvent(panelId, id, (event) => handleEvent(keyOf(panelId, id), event)),
        );
        await subscribeServer(panelId, id);
        subscribed.push({ panelId, id });
      }
    })();
    return () => {
      cancelled = true;
      for (const u of unlisteners) u();
      for (const s of subscribed) void unsubscribeServer(s.panelId, s.id);
    };
  });

  async function power(panelId: string, id: string, signal: PowerSignal) {
    try {
      await setPower(panelId, id, signal);
    } catch (e) {
      appendConsole(keyOf(panelId, id), `[feather] power "${signal}" failed: ${e}`);
    }
  }
</script>

<div class="servers">
  <div class="head">
    <div>
      <h2>Servers</h2>
      <p class="muted">Every server across your team's Pterodactyl panels — power, stats and console.</p>
    </div>
    <button class="ghost" onclick={onManage}>Manage panels</button>
  </div>

  {#if loading}
    <p class="muted center">Loading servers…</p>
  {:else if panels.length === 0}
    <p class="muted center empty">No panels connected. Add one under “Manage panels”.</p>
  {:else}
    {#each grouped as group (group.panel.id)}
      <section class="panel-group">
        <h3>{group.panel.name}</h3>
        {#if panelErrors[group.panel.id]}
          <p class="error">Could not reach this panel: {panelErrors[group.panel.id]}</p>
        {:else if group.servers.length === 0}
          <p class="muted small">No servers on this panel.</p>
        {:else}
          <div class="grid">
            {#each group.servers as entry (keyOf(entry.panelId, entry.server.identifier))}
              {@const k = keyOf(entry.panelId, entry.server.identifier)}
              <ServerCard
                server={entry.server}
                live={currentLive(k)}
                opsOnly
                onPower={(signal) => power(entry.panelId, entry.server.identifier, signal)}
                onOpenConsole={() => (openKey = k)}
              />
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#if openEntry}
  {@const k = keyOf(openEntry.panelId, openEntry.server.identifier)}
  <ConsoleView
    server={openEntry.server}
    live={currentLive(k)}
    lines={consoles[k] ?? []}
    onSend={(command) => sendConsoleCommand(openEntry.panelId, openEntry.server.identifier, command)}
    onClose={() => (openKey = null)}
  />
{/if}

<style>
  .servers {
    max-width: 1100px;
    margin: 8px auto 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 18px;
  }

  h2 {
    font-size: 18px;
    margin-bottom: 4px;
  }

  p {
    margin: 0;
    line-height: 1.5;
  }

  .panel-group {
    margin-bottom: 26px;
  }

  h3 {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin-bottom: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
  }

  .small {
    font-size: 13px;
  }

  .empty {
    padding: 40px 0;
  }
</style>
