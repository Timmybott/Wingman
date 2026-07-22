<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import { clearActivePanel, getProjectPath, removeLocalProject, setActivePanel } from "../api";
  import { auth } from "../auth.svelte";
  import { listPanels, listProjectDeletions, panelApiKey, type CloudPanel, type Team } from "../cloud";
  import { teamState } from "../team.svelte";
  import Footer from "./Footer.svelte";
  import Header from "./Header.svelte";
  import MembersScreen from "./MembersScreen.svelte";
  import PanelManager from "./PanelManager.svelte";
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

  let view = $state<"projects" | "panels" | "members" | "profile" | "team">("projects");
  // Which user's profile the "profile" view shows (may be someone else's).
  let profileUserId = $state<string | null>(null);
  // The tab to return to when leaving a profile/team page.
  let profileReturn = $state<"projects" | "panels" | "members">("projects");
  let panels = $state<CloudPanel[]>([]);
  let connected = $state<CloudPanel[]>([]);
  let connecting = $state(true);
  let managing = $state(false);
  let update = $state<Update | null>(null);
  // A pending "reveal this server in the Panels tab" request. Set when the
  // user clicks through from a project; ServersView scrolls to and highlights
  // the matching tile. A fresh object each time so repeat clicks re-trigger.
  let focusServer = $state<{ panelId: string; identifier: string } | null>(null);

  /** Jump from a project to its imported server's tile in the Panels tab. */
  function goToServer(panelId: string, identifier: string) {
    managing = false;
    view = "panels";
    focusServer = { panelId, identifier };
  }

  /** Remember which main tab we're on so a profile page can return to it. */
  function rememberReturn() {
    if (view === "projects" || view === "panels" || view === "members") {
      profileReturn = view;
    }
  }

  function openProfile(userId: string) {
    rememberReturn();
    profileUserId = userId;
    view = "profile";
  }

  function openTeamProfile() {
    rememberReturn();
    view = "team";
  }

  function onTeamUpdated(team: Team) {
    teamState.activeTeamName = team.name;
  }

  const teamId = $derived(teamState.activeTeamId);
  const connectedKey = $derived(connected.map((p) => p.id).join(","));

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
    } finally {
      connecting = false;
    }
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

  onMount(() => {
    void loadAndConnect();
    void processProjectDeletions();
    void (async () => {
      try {
        update = await check();
      } catch {
        update = null;
      }
    })();
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
    <button class:active={view === "projects"} onclick={() => (view = "projects")}>Projects</button>
    <button class:active={view === "panels"} onclick={() => (view = "panels")}>
      Panels{#if connected.length > 0}<span class="dot"></span>{/if}
    </button>
    <button class:active={view === "members"} onclick={() => (view = "members")}>Members</button>
  </nav>
  <main>
    {#if view === "profile"}
      {#if profileUserId}
        <UserProfile userId={profileUserId} onBack={() => (view = profileReturn)} />
      {/if}
    {:else if view === "team"}
      {#if teamId}
        <TeamProfile
          {teamId}
          onBack={() => (view = profileReturn)}
          onUpdated={onTeamUpdated}
        />
      {/if}
    {:else if view === "projects"}
      {#if teamId}
        <ProjectsScreen {teamId} onOpenServer={goToServer} />
      {/if}
    {:else if view === "members"}
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
            {focusServer}
            onManage={() => (managing = true)}
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
