<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    deployProject,
    deployStatus,
    listProjects,
    listServers,
    onDeployEvent,
    onServerEvent,
    rollbackProject,
    sendConsoleCommand,
    serverResources,
    setPower,
    subscribeServer,
    unsubscribeServer,
  } from "../api";
  import { appStatus } from "../appStatus.svelte";
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
        appendConsole(id, `[wingman] connection lost: ${event.data.reason}`);
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
        appendConsole(project.server_identifier, `[wingman] backup skipped: ${step.reason}`);
        return;
      case "done":
        appStatus.lastDeploy = { projectName: project.name, at: new Date(), files: step.files };
        void refreshGitStatus(project);
        break;
    }
    deploys[project.id] = step;
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
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      cancelled = true;
      for (const unlisten of serverUnlisteners) unlisten();
      for (const unlisten of deployUnlisteners.values()) unlisten();
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

  async function deploy(project: ProjectConfig) {
    deploys[project.id] = { step: "committing" };
    try {
      await deployProject(project.id);
    } catch (e) {
      deploys[project.id] = { step: "failed", message: String(e) };
    }
  }

  async function rollback(project: ProjectConfig, commitId: string) {
    deploys[project.id] = { step: "checking_out" };
    try {
      await rollbackProject(project.id, commitId);
    } catch (e) {
      deploys[project.id] = { step: "failed", message: String(e) };
    }
  }

  function projectSaved(saved: ProjectConfig) {
    const index = projects.findIndex((p) => p.id === saved.id);
    if (index >= 0) {
      projects[index] = saved;
    } else {
      projects.push(saved);
    }
    void watchProject(saved);
    dialogServer = null;
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
