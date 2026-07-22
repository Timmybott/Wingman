<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    checkRemoteDeploy,
    deployProject,
    deployStatus,
    listProjects,
    listServers,
    onDeployEvent,
    onServerEvent,
    projectHistory,
    pullProject,
    rollbackProject,
    sendConsoleCommand,
    serverResources,
    setPower,
    subscribeServer,
    unsubscribeServer,
  } from "../api";
  import { appStatus } from "../appStatus.svelte";
  import {
    findOrCreateProjectForServer,
    recordDeploy,
    type DeployKind,
  } from "../cloud";
  import { teamState } from "../team.svelte";
  import type {
    DeployStep,
    LiveState,
    PanelConfig,
    PowerSignal,
    ProjectConfig,
    Server,
    ServerEvent,
  } from "../types";
  import ConsoleView from "./ConsoleView.svelte";
  import FileBrowser from "./FileBrowser.svelte";
  import HistoryView from "./HistoryView.svelte";
  import LinkProjectDialog from "./LinkProjectDialog.svelte";
  import ServerCard from "./ServerCard.svelte";

  const CONSOLE_BUFFER_LINES = 500;
  /** How often each project checks the server for deploys from other devices. */
  const SYNC_POLL_MS = 30_000;

  let { panel }: { panel: PanelConfig } = $props();

  let servers = $state<Server[]>([]);
  let live = $state<Record<string, LiveState>>({});
  let consoles = $state<Record<string, string[]>>({});
  let openConsole = $state<string | null>(null);
  let projects = $state<ProjectConfig[]>([]);
  let deploys = $state<Record<string, DeployStep | null>>({});
  let dialogServer = $state<Server | null>(null);
  let historyProject = $state<ProjectConfig | null>(null);
  let filesServer = $state<Server | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  let cancelled = false;
  const serverUnlisteners: UnlistenFn[] = [];
  const deployUnlisteners = new Map<string, UnlistenFn>();
  // Tracks whether an in-flight engine run is a deploy or a rollback, so its
  // completion can be recorded to the cloud history. Pull import/sync runs are
  // deliberately not tracked (not recorded).
  const deployKinds = new Map<string, DeployKind>();

  const openServer = $derived(
    servers.find((server) => server.identifier === openConsole) ?? null,
  );

  function currentLive(id: string): LiveState {
    return live[id] ?? { state: null, stats: null, connected: false };
  }

  function projectFor(serverIdentifier: string): ProjectConfig | null {
    return projects.find((p) => p.server_identifier === serverIdentifier) ?? null;
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
        appendConsole(id, `[feather] connection lost: ${event.data.reason}`);
        break;
    }
  }

  function handleDeployEvent(project: ProjectConfig, step: DeployStep) {
    switch (step.step) {
      // Log-style events go to the console buffer, not the tile label.
      case "build_output":
        appendConsole(project.server_identifier, `[build] ${step.line}`);
        return;
      case "backup_skipped":
        appendConsole(project.server_identifier, `[feather] backup skipped: ${step.reason}`);
        return;
      case "done":
        appStatus.lastDeploy = { projectName: project.name, at: new Date(), files: step.files };
        void refreshGitStatus(project);
        break;
    }
    deploys[project.id] = step;

    // Record finished deploys/rollbacks to the shared cloud history.
    if (step.step === "done" || step.step === "failed") {
      const kind = deployKinds.get(project.id);
      if (kind) {
        deployKinds.delete(project.id);
        void recordDeployOutcome(project, kind, step);
      }
    }
  }

  /** Best-effort: write a deploy/rollback outcome to the project's cloud history. */
  async function recordDeployOutcome(
    project: ProjectConfig,
    kind: DeployKind,
    step: DeployStep,
  ) {
    const teamId = teamState.activeTeamId;
    if (!teamId) return;
    try {
      const cloudProject = await findOrCreateProjectForServer(
        teamId,
        project.panel_id,
        project.server_identifier,
        project.name,
      );
      if (step.step === "done") {
        let commit: string | null = null;
        let commitSummary: string | null = null;
        try {
          const [head] = await projectHistory(project.id, 1);
          if (head) {
            commit = head.short_id;
            commitSummary = head.summary;
          }
        } catch {
          // History is a nicety; record the deploy regardless.
        }
        await recordDeploy({
          projectId: cloudProject.id,
          kind,
          status: "success",
          commit,
          commitSummary,
          files: step.files,
        });
      } else if (step.step === "failed") {
        await recordDeploy({
          projectId: cloudProject.id,
          kind,
          status: "failed",
          message: step.message,
        });
      }
    } catch (e) {
      // Never let history recording disrupt the deploy flow.
      console.error("failed to record deploy history:", e);
    }
  }

  /** Feed the footer's "N commits since last deploy". */
  async function refreshGitStatus(project: ProjectConfig) {
    try {
      const ds = await deployStatus(project.id);
      if (ds.commits_since !== null) {
        appStatus.gitStatus = { projectName: project.name, commitsSince: ds.commits_since };
      }
    } catch {
      // Footer info only — never disruptive.
    }
  }

  /** Register the deploy-event listener for a project (idempotent). */
  async function watchProject(project: ProjectConfig) {
    if (deployUnlisteners.has(project.id)) return;
    const unlisten = await onDeployEvent(project.id, (step) =>
      handleDeployEvent(project, step),
    );
    if (cancelled) {
      unlisten();
      return;
    }
    deployUnlisteners.set(project.id, unlisten);
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
    let syncTimer: ReturnType<typeof setInterval> | undefined;
    (async () => {
      try {
        [servers, projects] = await Promise.all([listServers(), listProjects()]);
        for (const project of projects) {
          await watchProject(project);
          void refreshGitStatus(project);
        }
        for (const server of servers) {
          if (cancelled) break;
          const id = server.identifier;
          void prefill(id);
          // Listen before subscribing so the initial burst is not missed.
          serverUnlisteners.push(await onServerEvent(id, (event) => handleEvent(id, event)));
          await subscribeServer(id);
        }
        void syncCheck();
        syncTimer = setInterval(syncCheck, SYNC_POLL_MS);
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      cancelled = true;
      if (syncTimer) clearInterval(syncTimer);
      for (const unlisten of serverUnlisteners) unlisten();
      for (const unlisten of deployUnlisteners.values()) unlisten();
      for (const server of servers) void unsubscribeServer(server.identifier);
    };
  });

  async function power(id: string, signal: PowerSignal) {
    try {
      await setPower(id, signal);
    } catch (e) {
      appendConsole(id, `[feather] power "${signal}" failed: ${e}`);
    }
  }

  async function deploy(project: ProjectConfig) {
    deploys[project.id] = { step: "committing" };
    deployKinds.set(project.id, "deploy");
    try {
      await deployProject(project.id);
    } catch (e) {
      const step: DeployStep = { step: "failed", message: String(e) };
      deploys[project.id] = step;
      deployKinds.delete(project.id);
      void recordDeployOutcome(project, "deploy", step);
    }
  }

  function deployRunning(projectId: string): boolean {
    const step = deploys[projectId]?.step;
    return step !== undefined && step !== "done" && step !== "failed";
  }

  async function pull(project: ProjectConfig, mode: "import" | "sync") {
    deploys[project.id] = { step: "downloading", percent: 0 };
    try {
      await pullProject(project.id, mode);
    } catch (e) {
      deploys[project.id] = { step: "failed", message: String(e) };
    }
  }

  /** Multi-device sync: pull deploys made on other devices automatically. */
  const dirtyNotified = new Set<string>();
  async function syncCheck() {
    for (const project of [...projects]) {
      if (cancelled || deployRunning(project.id)) continue;
      try {
        const info = await checkRemoteDeploy(project.id);
        if (!info.newer) {
          dirtyNotified.delete(project.id);
          continue;
        }
        if (info.dirty) {
          if (!dirtyNotified.has(project.id)) {
            dirtyNotified.add(project.id);
            appendConsole(
              project.server_identifier,
              "[feather] a newer deploy from another device exists — commit or deploy your local changes to sync",
            );
          }
          continue;
        }
        dirtyNotified.delete(project.id);
        appendConsole(
          project.server_identifier,
          "[feather] newer deploy from another device detected — syncing local folder",
        );
        await pull(project, "sync");
      } catch {
        // Polling is best effort; the next tick retries.
      }
    }
  }

  async function rollback(project: ProjectConfig, commitId: string) {
    deploys[project.id] = { step: "checking_out" };
    deployKinds.set(project.id, "rollback");
    try {
      await rollbackProject(project.id, commitId);
    } catch (e) {
      const step: DeployStep = { step: "failed", message: String(e) };
      deploys[project.id] = step;
      deployKinds.delete(project.id);
      void recordDeployOutcome(project, "rollback", step);
    }
  }

  function projectSaved(saved: ProjectConfig) {
    const index = projects.findIndex((p) => p.id === saved.id);
    const isNew = index < 0;
    if (isNew) {
      projects.push(saved);
    } else {
      projects[index] = saved;
    }
    dialogServer = null;
    void (async () => {
      await watchProject(saved);
      if (isNew) {
        // Fill the fresh link with the server's current files. The engine
        // skips on its own when the folder already has content.
        appendConsole(
          saved.server_identifier,
          "[feather] importing current server files into the linked folder…",
        );
        await pull(saved, "import");
      }
    })();
  }

  function projectUnlinked(projectId: string) {
    projects = projects.filter((p) => p.id !== projectId);
    deployUnlisteners.get(projectId)?.();
    deployUnlisteners.delete(projectId);
    delete deploys[projectId];
    dialogServer = null;
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
      {@const project = projectFor(server.identifier)}
      <ServerCard
        {server}
        live={currentLive(server.identifier)}
        {project}
        deploy={project ? (deploys[project.id] ?? null) : null}
        onPower={(signal) => power(server.identifier, signal)}
        onOpenConsole={() => (openConsole = server.identifier)}
        onDeploy={() => project && deploy(project)}
        onConfigureProject={() => (dialogServer = server)}
        onOpenHistory={() => (historyProject = project)}
        onOpenFiles={() => (filesServer = server)}
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

{#if dialogServer}
  <LinkProjectDialog
    server={dialogServer}
    project={projectFor(dialogServer.identifier)}
    panelId={panel.id}
    onSaved={projectSaved}
    onUnlinked={projectUnlinked}
    onClose={() => (dialogServer = null)}
  />
{/if}

{#if filesServer}
  <FileBrowser server={filesServer} onClose={() => (filesServer = null)} />
{/if}

{#if historyProject}
  {@const project = historyProject}
  <HistoryView
    {project}
    onRollback={(commitId) => {
      historyProject = null;
      void rollback(project, commitId);
    }}
    onChanged={() => refreshGitStatus(project)}
    onClose={() => (historyProject = null)}
  />
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
  }
</style>
