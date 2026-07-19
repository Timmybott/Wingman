<script lang="ts">
  import { cpuPercent, formatBytes, formatMib, memoryPercent } from "../format";
  import type { PowerState, Server, ServerStats } from "../types";

  let { server, stats }: { server: Server; stats?: ServerStats } = $props();

  const powerState = $derived<PowerState | "unknown">(
    stats?.current_state ?? "unknown",
  );

  const statusLabel = $derived(
    server.is_suspended
      ? "Suspended"
      : server.is_installing
        ? "Installing"
        : { running: "Online", starting: "Starting", stopping: "Stopping", offline: "Offline", unknown: "…" }[
            powerState
          ],
  );

  const dotClass = $derived(
    server.is_suspended
      ? "offline"
      : { running: "online", starting: "busy", stopping: "busy", offline: "offline", unknown: "unknown" }[
          powerState
        ],
  );

  const cpu = $derived(stats ? cpuPercent(stats.resources.cpu_absolute, server.limits.cpu) : null);
  const memory = $derived(
    stats ? memoryPercent(stats.resources.memory_bytes, server.limits.memory) : null,
  );

  const cpuLabel = $derived(
    stats
      ? server.limits.cpu > 0
        ? `${stats.resources.cpu_absolute.toFixed(1)} / ${server.limits.cpu}%`
        : `${stats.resources.cpu_absolute.toFixed(1)}%`
      : "–",
  );

  const memoryLabel = $derived(
    stats
      ? server.limits.memory > 0
        ? `${formatBytes(stats.resources.memory_bytes)} / ${formatMib(server.limits.memory)}`
        : formatBytes(stats.resources.memory_bytes)
      : "–",
  );
</script>

<article class="card">
  <div class="top">
    <div>
      <h3>{server.name}</h3>
      <span class="muted node">{server.node}</span>
    </div>
    <span class="status"><span class="dot {dotClass}"></span> {statusLabel}</span>
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
    <!-- Deploy is the heart of the app (M3); power control lands in M2.
         Shown disabled so the dashboard already has its final shape. -->
    <button class="primary deploy" disabled title="Deploy arrives in milestone M3">
      Deploy
    </button>
    <button disabled title="Power actions arrive in milestone M2">⏻</button>
    <button class="ghost" disabled title="Console arrives in milestone M2">Console</button>
    <button class="ghost" disabled title="History arrives in milestone M4">History</button>
    <button class="ghost" disabled title="Files arrive in milestone M5">Files</button>
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
    gap: 6px;
  }

  .deploy {
    flex: 1;
  }
</style>
