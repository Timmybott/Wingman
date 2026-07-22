<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import { clearActivePanel, setActivePanel } from "../api";
  import { listPanels, panelApiKey, type CloudPanel } from "../cloud";
  import { teamState } from "../team.svelte";
  import Dashboard from "./Dashboard.svelte";
  import Footer from "./Footer.svelte";
  import Header from "./Header.svelte";
  import PanelManager from "./PanelManager.svelte";
  import ProjectsScreen from "./ProjectsScreen.svelte";
  import UpdateDialog from "./UpdateDialog.svelte";

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

  let view = $state<"projects" | "panels">("projects");
  let panels = $state<CloudPanel[]>([]);
  let activePanel = $state<CloudPanel | null>(null);
  let loading = $state(true);
  let activating = $state(false);
  let error = $state<string | null>(null);
  let update = $state<Update | null>(null);

  const teamId = $derived(teamState.activeTeamId);
  const lastPanelKey = $derived(teamId ? `feather.activePanel.${teamId}` : null);

  async function loadPanels() {
    if (!teamId) return;
    loading = true;
    error = null;
    try {
      panels = await listPanels(teamId);
      // Reconnect the panel used last on this team, if it still exists.
      const lastId = lastPanelKey ? localStorage.getItem(lastPanelKey) : null;
      const last = lastId ? panels.find((p) => p.id === lastId) : undefined;
      if (last) await activate(last);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  /** Load a panel's decrypted key from the cloud and connect the core to it. */
  async function activate(panel: CloudPanel) {
    activating = true;
    error = null;
    try {
      const key = await panelApiKey(panel.id);
      await setActivePanel(panel.base_url, key);
      activePanel = panel;
      if (lastPanelKey) localStorage.setItem(lastPanelKey, panel.id);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      activating = false;
    }
  }

  async function disconnect() {
    try {
      await clearActivePanel();
    } catch (e) {
      console.error("failed to disconnect panel:", e);
    }
    activePanel = null;
    if (lastPanelKey) localStorage.removeItem(lastPanelKey);
    // Reflect any panels added/removed while a dashboard was open.
    await loadPanels();
  }

  onMount(() => {
    void loadPanels();
    // Update check is best effort: in dev builds or before the updater
    // keypair is configured this simply fails silently.
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
    panel={activePanel}
    {userEmail}
    {teamName}
    {onSwitchTeam}
    {onLogout}
    onDisconnect={disconnect}
  />
  <nav class="tabs">
    <button class:active={view === "projects"} onclick={() => (view = "projects")}>Projects</button>
    <button class:active={view === "panels"} onclick={() => (view = "panels")}>
      Panels{#if activePanel}<span class="dot"></span>{/if}
    </button>
  </nav>
  <main>
    {#if view === "projects"}
      {#if teamId}
        <ProjectsScreen {teamId} />
      {/if}
    {:else if loading}
      <p class="muted center">Loading…</p>
    {:else if activePanel}
      <Dashboard panel={activePanel} />
    {:else if teamId}
      <PanelManager
        {panels}
        {teamId}
        {activating}
        activateError={error}
        onActivate={activate}
        onChanged={loadPanels}
      />
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
