<script lang="ts">
  import { cpuPercent, formatBytes, formatMib, memoryPercent } from "../format";
  import type { LiveState, PowerSignal, PowerState, Server } from "../types";

  let {
    server,
    live,
    onPower,
    onOpenConsole,
  }: {
    server: Server;
    live: LiveState;
    onPower: (signal: PowerSignal) => Promise<void>;
    onOpenConsole: () => void;
  } = $props();

  let busy = $state(false);
  let killArmed = $state(false);
  let killTimer: ReturnType<typeof setTimeout> | undefined;

  const powerState = $derived<PowerState | "unknown">(live.state ?? "unknown");
  const stats = $derived(live.stats);

  const statusLabel = $derived(
    server.is_suspended
      ? "Suspended"
      : server.is_installing
        ? "Installing"
        : {
            running: "Online",
            starting: "Starting",
            stopping: "Stopping",
            offline: "Offline",
            unknown: "…",
          }[powerState],
  );

  const dotClass = $derived(
    server.is_suspended
      ? "offline"
      : {
          running: "online",
          starting: "busy",
          stopping: "busy",
          offline: "offline",
          unknown: "unknown",
        }[powerState],
  );

  const cpu = $derived(stats ? cpuPercent(stats.cpu_absolute, server.limits.cpu) : null);
  const memory = $derived(
    stats ? memoryPercent(stats.memory_bytes, server.limits.memory) : null,
  );

  const cpuLabel = $derived(
    stats
      ? server.limits.cpu > 0
        ? `${stats.cpu_absolute.toFixed(1)} / ${server.limits.cpu}%`
        : `${stats.cpu_absolute.toFixed(1)}%`
      : "–",
  );

  const memoryLabel = $derived(
    stats
      ? server.limits.memory > 0
        ? `${formatBytes(stats.memory_bytes)} / ${formatMib(server.limits.memory)}`
        : formatBytes(stats.memory_bytes)
      : "–",
  );

  const canStart = $derived(powerState === "offline");
  const canStop = $derived(powerState === "running");
  const showKill = $derived(powerState !== "offline" && powerState !== "unknown");

  async function power(signal: PowerSignal) {
    busy = true;
    try {
      await onPower(signal);
    } finally {
      busy = false;
      disarmKill();
    }
  }

  // Kill is destructive (no graceful shutdown) — require a second click.
  function killClick() {
    if (!killArmed) {
      killArmed = true;
      killTimer = setTimeout(() => (killArmed = false), 3000);
      return;
    }
    void power("kill");
  }

  function disarmKill() {
    killArmed = false;
    if (killTimer) clearTimeout(killTimer);
  }
</script>

<article class="card">
  <div class="top">
    <div>
      <h3>{server.name}</h3>
      <span class="muted node">{server.node}</span>
    </div>
    <span class="status" title={live.connected ? "Live connection" : "No live connection"}>
      <span class="dot {dotClass}"></span>
      {statusLabel}
    </span>
  </div>

  <div class="meters">
    <div class="meter">
      <div class="meter-head">
        <span class="muted">CPU</span>
        <span>{cpuLabel}</span>
      </div>
      <div class="bar"><div class="fill" style="width: {cpu ?? 0}%"></div></div>
    </div>
    <div class="meter">
      <div class="meter-head">
        <span class="muted">RAM</span>
        <span>{memoryLabel}</span>
      </div>
      <div class="bar"><div class="fill" style="width: {memory ?? 0}%"></div></div>
    </div>
  </div>

  <div class="card-actions">
    <button class="primary deploy" disabled title="Deploy arrives in milestone M3">
      Deploy
    </button>
    {#if canStart}
      <button onclick={() => power("start")} disabled={busy} title="Start server">▶</button>
    {:else}
      <button
        onclick={() => power("stop")}
        disabled={busy || !canStop}
        title="Stop server"
      >
        ⏹
      </button>
    {/if}
    <button
      onclick={() => power("restart")}
      disabled={busy || !canStop}
      title="Restart server"
    >
      ⟳
    </button>
    {#if showKill}
      <button class="kill" class:armed={killArmed} onclick={killClick} disabled={busy}>
        {killArmed ? "Sure?" : "Kill"}
      </button>
    {/if}
    <button class="ghost" onclick={onOpenConsole}>Console</button>
  </div>
</article>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }

  h3 {
    font-size: 15px;
  }

  .node {
    font-size: 12px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    white-space: nowrap;
  }

  .meters {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .meter-head {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    margin-bottom: 3px;
  }

  .bar {
    height: 5px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.6s ease;
  }

  .card-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
  }

  .deploy {
    flex: 1;
    min-width: 90px;
  }

  .kill {
    color: var(--danger);
  }

  .kill.armed {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
</style>
