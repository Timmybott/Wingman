<script lang="ts">
  import { cpuPercent, formatBytes, formatMib, memoryPercent } from "../format";
  import type {
    DeployStep,
    LiveState,
    PowerSignal,
    PowerState,
    ProjectConfig,
    Server,
  } from "../types";

  let {
    server,
    live,
    project,
    deploy,
    onPower,
    onOpenConsole,
    onDeploy,
    onConfigureProject,
    onOpenHistory,
    onOpenFiles,
  }: {
    server: Server;
    live: LiveState;
    project: ProjectConfig | null;
    deploy: DeployStep | null;
    onPower: (signal: PowerSignal) => Promise<void>;
    onOpenConsole: () => void;
    onDeploy: () => void;
    onConfigureProject: () => void;
    onOpenHistory: () => void;
    onOpenFiles: () => void;
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

  // Real panels may return null limits (= unlimited); treat like 0.
  const cpuLimit = $derived(server.limits.cpu ?? 0);
  const memoryLimit = $derived(server.limits.memory ?? 0);

  const cpu = $derived(stats ? cpuPercent(stats.cpu_absolute, cpuLimit) : null);
  const memory = $derived(stats ? memoryPercent(stats.memory_bytes, memoryLimit) : null);

  const cpuLabel = $derived(
    stats
      ? cpuLimit > 0
        ? `${stats.cpu_absolute.toFixed(1)} / ${cpuLimit}%`
        : `${stats.cpu_absolute.toFixed(1)}%`
      : "–",
  );

  const memoryLabel = $derived(
    stats
      ? memoryLimit > 0
        ? `${formatBytes(stats.memory_bytes)} / ${formatMib(memoryLimit)}`
        : formatBytes(stats.memory_bytes)
      : "–",
  );

  const canStart = $derived(powerState === "offline");
  const canStop = $derived(powerState === "running");
  const showKill = $derived(powerState !== "offline" && powerState !== "unknown");

  // Deploy state shown directly on the tile (spec: "Backup erstellt · Upload 68 %").
  const deployRunning = $derived(
    deploy !== null && deploy.step !== "done" && deploy.step !== "failed",
  );

  const deployLabel = $derived.by(() => {
    if (!deploy) return null;
    switch (deploy.step) {
      case "committing":
        return "Committing…";
      case "checking_out":
        return "Checking out…";
      case "building":
        return "Building…";
      case "backing_up":
        return "Backing up…";
      case "scanning":
        return "Scanning…";
      case "packing":
        return `Packing ${deploy.files} files…`;
      case "uploading":
        return `Uploading ${deploy.percent} %`;
      case "downloading":
        return `Downloading ${deploy.percent} %`;
      case "importing":
        return "Importing files…";
      case "extracting":
        return "Extracting…";
      case "cleaning_up":
        return "Cleaning up…";
      case "restarting":
        return "Restarting…";
      default:
        return null;
    }
  });

  const deployPercent = $derived(
    deploy?.step === "uploading" || deploy?.step === "downloading" ? deploy.percent : null,
  );

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

  {#if deployRunning && deployLabel}
    <div class="deploy-progress">
      <div class="deploy-label">
        <span>{deployLabel}</span>
      </div>
      <div class="bar">
        <div
          class="fill deploy-fill"
          class:indeterminate={deployPercent === null}
          style="width: {deployPercent ?? 100}%"
        ></div>
      </div>
    </div>
  {:else if deploy?.step === "failed"}
    <p class="deploy-note error" title={deploy.message}>Deploy failed: {deploy.message}</p>
  {:else if deploy?.step === "done"}
    <p class="deploy-note ok">
      Deployed ✓ {deploy.files} files{deploy.deleted > 0 ? `, ${deploy.deleted} removed` : ""}
    </p>
  {/if}

  <div class="card-actions">
    {#if project}
      <button
        class="primary deploy"
        onclick={onDeploy}
        disabled={deployRunning}
        title="Deploy {project.name} to this server"
      >
        Deploy
      </button>
      <button
        class="ghost"
        onclick={onConfigureProject}
        disabled={deployRunning}
        title="Project settings"
      >
        ⚙
      </button>
    {:else}
      <button class="primary deploy" onclick={onConfigureProject} title="Link a local project folder">
        Link project…
      </button>
    {/if}
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
    <button
      class="ghost"
      onclick={onOpenHistory}
      disabled={!project}
      title={project ? "Commit history & rollback" : "Link a project first"}
    >
      History
    </button>
    <button class="ghost" onclick={onOpenFiles} title="Browse server files">Files</button>
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

  .deploy-progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .deploy-label {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--accent);
  }

  .deploy-fill.indeterminate {
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.35;
    }
    50% {
      opacity: 1;
    }
  }

  .deploy-note {
    margin: 0;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
