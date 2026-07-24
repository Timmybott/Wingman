<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import {
    clearActivePanel,
    getProjectPath,
    listProjectPaths,
    removeLocalProject,
    setActivePanel,
  } from "../api";
  import { checkProject, projectConfig } from "../sync.svelte";
  import { auth } from "../auth.svelte";
  import {
    getProject,
    getTeam,
    listMembers,
    listPanels,
    listProjectDeletions,
    listProjects,
    panelApiKey,
    type CloudPanel,
    type CloudProject,
    type Team,
    type TeamMember,
  } from "../cloud";
  import { teamState } from "../team.svelte";
  import Footer from "./Footer.svelte";
  import Header from "./Header.svelte";
  import MembersScreen from "./MembersScreen.svelte";
  import PanelManager from "./PanelManager.svelte";
  import ProjectDetail from "./ProjectDetail.svelte";
  import ProjectsScreen from "./ProjectsScreen.svelte";
  import ServersView from "./ServersView.svelte";
  import TeamProfile from "./TeamProfile.svelte";
  import UpdateDialog from "./UpdateDialog.svelte";
  import UserProfile from "./UserProfile.svelte";

  let {
    userEmail,
    teamName,
    onSwitchTeam,
    onLogout,
  }: {
    userEmail: string;
    teamName: string;
    onSwitchTeam: () => void;
    onLogout: () => void;
  } = $props();

  // A real navigation stack: the last entry is the page on screen, and Back
  // pops it, so every page returns to wherever it was actually opened from —
  // a profile opened from a project returns to that project, not the list.
  type Route =
    | { kind: "projects" }
    | { kind: "panels"; focusServer?: { panelId: string; identifier: string } }
    | { kind: "members" }
    | { kind: "project"; projectId: string }
    | { kind: "user"; userId: string }
    | { kind: "team"; teamId: string };

  let stack = $state<Route[]>([{ kind: "projects" }]);
  const current = $derived(stack[stack.length - 1]);
  const canBack = $derived(stack.length > 1);

  function push(route: Route) {
    stack = [...stack, route];
  }
  function back() {
    if (stack.length > 1) stack = stack.slice(0, -1);
  }
  /** Switch to a top-level tab — resets the stack to that root. */
  function openTab(kind: "projects" | "panels" | "members") {
    managing = false;
    stack = [{ kind }];
  }

  function openProfile(userId: string) {
    push({ kind: "user", userId });
  }
  /** Open a team's page — defaults to the active team. */
  function openTeamProfile(id?: string) {
    const t = id ?? teamId;
    if (t) push({ kind: "team", teamId: t });
  }
  function goToProject(projectId: string) {
    managing = false;
    push({ kind: "project", projectId });
  }
  /** Jump from a project to its imported server's tile in the Panels view. */
  function goToServer(panelId: string, identifier: string) {
    managing = false;
    push({ kind: "panels", focusServer: { panelId, identifier } });
  }

  let panels = $state<CloudPanel[]>([]);
  let connected = $state<CloudPanel[]>([]);
  let projects = $state<CloudProject[]>([]);
  let members = $state<TeamMember[]>([]);
  let connecting = $state(true);
  let managing = $state(false);
  let update = $state<Update | null>(null);

  // A project belonging to another team the user is also on: viewable read-only
  // (all Feather projects are open source), loaded on demand since the shell
  // only keeps the active team's projects/panels/members in memory.
  let foreignProject = $state<CloudProject | null>(null);
  let foreignMembers = $state<TeamMember[]>([]);
  let foreignPanels = $state<CloudPanel[]>([]);
  let foreignTeamName = $state("");
  let foreignLoading = $state(false);
  let foreignError = $state<string | null>(null);
  // The project id we've started (or finished) a foreign load for — guards the
  // loader effect against re-entry without becoming a dependency loop.
  let foreignAttemptId = $state<string | null>(null);

  const teamId = $derived(teamState.activeTeamId);
  const connectedKey = $derived(connected.map((p) => p.id).join(","));
  const focusServer = $derived(
    current.kind === "panels" ? (current.focusServer ?? null) : null,
  );
  const activeProject = $derived.by(() => {
    if (current.kind !== "project") return null;
    const own = projects.find((p) => p.id === current.projectId);
    if (own) return own;
    if (foreignProject && foreignProject.id === current.projectId) return foreignProject;
    return null;
  });
  // Writable only for a project of the currently active team. A project from
  // another team is read-only: view its files, history and issues, comment and
  // open issues, but never deploy, commit, roll back or change settings.
  const canWriteProject = $derived(!!activeProject && activeProject.team_id === teamId);
  // Opening issues and commenting need actual membership of the project's team
  // (the RPCs enforce it): the active team, or another of the viewer's teams.
  // A stranger's project reached via a public profile is view-only.
  const canInteractProject = $derived.by(() => {
    if (!activeProject) return false;
    if (activeProject.team_id === teamId) return true;
    const uid = auth.user?.id ?? null;
    return foreignMembers.some((m) => m.user_id === uid);
  });
  const projectMembers = $derived(canWriteProject ? members : foreignMembers);
  const projectPanels = $derived(canWriteProject ? panels : foreignPanels);
  const projectTeamName = $derived(
    canWriteProject ? (teamState.activeTeamName ?? teamName) : foreignTeamName,
  );

  /** Load a project (and just enough context) from another of the user's teams. */
  async function loadForeignProject(id: string) {
    foreignLoading = true;
    foreignError = null;
    try {
      const proj = await getProject(id);
      const [team, tmembers, tpanels] = await Promise.all([
        getTeam(proj.team_id).catch(() => null),
        listMembers(proj.team_id).catch(() => [] as TeamMember[]),
        listPanels(proj.team_id).catch(() => [] as CloudPanel[]),
      ]);
      foreignTeamName = team?.name ?? "";
      foreignMembers = tmembers;
      foreignPanels = tpanels;
      // Connect the project's panel (in memory) so its files and server state
      // are reachable for read-only browsing, if it isn't already connected.
      if (proj.panel_id && !connected.some((p) => p.id === proj.panel_id)) {
        const panel = tpanels.find((p) => p.id === proj.panel_id);
        if (panel) {
          try {
            const key = await panelApiKey(panel.id);
            await setActivePanel(panel.id, panel.base_url, key);
          } catch (e) {
            console.error("could not connect foreign panel:", e);
          }
        }
      }
      foreignProject = proj;
    } catch (e) {
      foreignError = String(e instanceof Error ? e.message : e);
    } finally {
      foreignLoading = false;
    }
  }

  // When a project route points at something outside the active team (and the
  // active team's projects have finished loading), fetch it read-only.
  $effect(() => {
    if (current.kind !== "project") return;
    const id = current.projectId;
    if (projects.some((p) => p.id === id) || connecting) return;
    if (foreignAttemptId === id) return;
    foreignAttemptId = id;
    void loadForeignProject(id);
  });

  function onTeamUpdated(team: Team) {
    // Only reflect a rename in the header if it's the currently active team.
    if (team.id === teamId) teamState.activeTeamName = team.name;
  }

  /** Connect every team panel (decrypt its key, hand it to the core). */
  async function loadAndConnect() {
    if (!teamId) return;
    connecting = true;
    try {
      const all = await listPanels(teamId);
      // Disconnect panels that were removed.
      for (const prev of connected) {
        if (!all.some((p) => p.id === prev.id)) {
          try {
            await clearActivePanel(prev.id);
          } catch {
            // ignore
          }
        }
      }
      const ok: CloudPanel[] = [];
      for (const panel of all) {
        try {
          const key = await panelApiKey(panel.id);
          await setActivePanel(panel.id, panel.base_url, key);
          ok.push(panel);
        } catch (e) {
          console.error(`could not connect panel ${panel.name}:`, e);
        }
      }
      panels = all;
      connected = ok;
      // Load projects and members too: projects mark servers in the Panels view
      // and back the Projects list, members name a project's creator.
      try {
        projects = await listProjects(teamId);
      } catch {
        projects = [];
      }
      try {
        members = await listMembers(teamId);
      } catch {
        members = [];
      }
    } finally {
      connecting = false;
    }
  }

  function onProjectCreated(project: CloudProject) {
    projects = [...projects, project];
    push({ kind: "project", projectId: project.id });
  }
  function onProjectChanged(updated: CloudProject) {
    const i = projects.findIndex((p) => p.id === updated.id);
    if (i >= 0) projects[i] = updated;
  }
  function onProjectDeleted(id: string) {
    projects = projects.filter((p) => p.id !== id);
    back();
  }

  /**
   * Act on "delete everywhere" tombstones: for any project the team has
   * tombstoned that still has a local folder on this device, delete that
   * folder and forget the project. Best effort — never blocks the app.
   */
  async function processProjectDeletions() {
    if (!teamId) return;
    try {
      const tombstoned = await listProjectDeletions(teamId);
      for (const projectId of tombstoned) {
        const path = await getProjectPath(projectId);
        if (path) await removeLocalProject(projectId, true);
      }
    } catch (e) {
      console.error("could not process project deletions:", e);
    }
  }

  /**
   * App-wide background sync: for every project bound to a local folder on this
   * device (in the active team, whose panel is connected), check for a newer
   * team deploy and pull it in when safe. Runs while the app is open — no need
   * to have the project's Deploy tab open — and once on launch so a teammate
   * who was offline catches up on start.
   */
  async function runSyncSweep() {
    if (!teamId || connecting) return;
    let paths: Record<string, string>;
    try {
      paths = await listProjectPaths();
    } catch {
      return;
    }
    for (const project of projects) {
      const localPath = paths[project.id];
      if (!localPath || !project.panel_id || !project.server_identifier) continue;
      void checkProject(projectConfig(project, localPath));
    }
  }

  let syncTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    void (async () => {
      await loadAndConnect();
      await runSyncSweep();
    })();
    void processProjectDeletions();
    void (async () => {
      try {
        update = await check();
      } catch {
        update = null;
      }
    })();
    // Keep every teammate current while the app is open (foreground or not).
    syncTimer = setInterval(() => void runSyncSweep(), 25_000);
    return () => {
      if (syncTimer) clearInterval(syncTimer);
    };
  });

  async function installUpdate() {
    if (!update) return;
    await update.downloadAndInstall();
    await relaunch();
  }
</script>

<div class="shell">
  <Header
    {userEmail}
    {teamName}
    onOpenProfile={() => auth.user && openProfile(auth.user.id)}
    onOpenTeam={openTeamProfile}
    {onSwitchTeam}
    {onLogout}
  />
  <nav class="tabs">
    <button class:active={current.kind === "projects"} onclick={() => openTab("projects")}>
      Projects
    </button>
    <button class:active={current.kind === "panels"} onclick={() => openTab("panels")}>
      Panels{#if connected.length > 0}<span class="dot"></span>{/if}
    </button>
    <button class:active={current.kind === "members"} onclick={() => openTab("members")}>
      Members
    </button>
  </nav>
  <main>
    {#if current.kind === "user"}
      <UserProfile
        userId={current.userId}
        onBack={back}
        onOpenTeam={openTeamProfile}
        onOpenProject={goToProject}
      />
    {:else if current.kind === "team"}
      <TeamProfile
        teamId={current.teamId}
        onBack={back}
        onUpdated={onTeamUpdated}
        onOpenProfile={openProfile}
        onOpenProject={goToProject}
      />
    {:else if current.kind === "project"}
      {#if activeProject}
        <ProjectDetail
          project={activeProject}
          panels={projectPanels}
          members={projectMembers}
          teamName={projectTeamName}
          canWrite={canWriteProject}
          canInteract={canInteractProject}
          onBack={back}
          onChanged={onProjectChanged}
          onDeleted={onProjectDeleted}
          onOpenServer={goToServer}
          onOpenTeam={openTeamProfile}
          onOpenProfile={openProfile}
        />
      {:else if connecting || foreignLoading || foreignAttemptId !== current.projectId}
        <p class="muted center">Loading project…</p>
      {:else if foreignError}
        <p class="error center">Couldn't open this project: {foreignError}</p>
      {:else}
        <p class="muted center">This project is no longer available.</p>
      {/if}
    {:else if current.kind === "projects"}
      {#if teamId}
        <ProjectsScreen
          {teamId}
          {projects}
          {panels}
          loading={connecting}
          onOpenProject={goToProject}
          onCreated={onProjectCreated}
        />
      {/if}
    {:else if current.kind === "members"}
      {#if teamId}
        <MembersScreen {teamId} onOpenProfile={openProfile} />
      {/if}
    {:else if teamId}
      {#if managing}
        <PanelManager
          {teamId}
          {panels}
          onChanged={loadAndConnect}
          onClose={() => (managing = false)}
        />
      {:else if connecting}
        <p class="muted center">Connecting to panels…</p>
      {:else}
        {#key connectedKey}
          <ServersView
            panels={connected.map((p) => ({ id: p.id, name: p.name }))}
            {projects}
            {focusServer}
            onBack={canBack ? back : undefined}
            onManage={() => (managing = true)}
            onOpenProject={goToProject}
          />
        {/key}
      {/if}
    {/if}
  </main>
  <Footer />
</div>

{#if update}
  <UpdateDialog {update} onInstall={installUpdate} onLater={() => (update = null)} />
{/if}

<style>
  .tabs {
    display: flex;
    gap: 4px;
    padding: 0 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 10px 12px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .tabs .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #34d399;
  }
</style>
