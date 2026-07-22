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
  <main>
    {#if loading}
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
