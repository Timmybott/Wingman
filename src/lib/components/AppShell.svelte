<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import { getPanel } from "../api";
  import type { PanelConfig } from "../types";
  import Dashboard from "./Dashboard.svelte";
  import Footer from "./Footer.svelte";
  import Header from "./Header.svelte";
  import SetupScreen from "./SetupScreen.svelte";
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

  let panel = $state<PanelConfig | null>(null);
  let loading = $state(true);
  let update = $state<Update | null>(null);

  onMount(async () => {
    try {
      panel = await getPanel();
    } catch (error) {
      console.error("failed to load panel config:", error);
    } finally {
      loading = false;
    }
    // Update check is best effort: in dev builds or before the updater
    // keypair is configured this simply fails silently.
    try {
      update = await check();
    } catch {
      update = null;
    }
  });

  async function installUpdate() {
    if (!update) return;
    await update.downloadAndInstall();
    await relaunch();
  }
</script>

<div class="shell">
  <Header
    {panel}
    {userEmail}
    {teamName}
    {onSwitchTeam}
    {onLogout}
    onDisconnect={() => (panel = null)}
  />
  <main>
    {#if loading}
      <p class="muted center">Loading…</p>
    {:else if panel}
      <Dashboard {panel} />
    {:else}
      <SetupScreen onConnected={(connected) => (panel = connected)} />
    {/if}
  </main>
  <Footer />
</div>

{#if update}
  <UpdateDialog {update} onInstall={installUpdate} onLater={() => (update = null)} />
{/if}
