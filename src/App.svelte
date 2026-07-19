<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import { getPanel } from "./lib/api";
  import type { PanelConfig } from "./lib/types";
  import Dashboard from "./lib/components/Dashboard.svelte";
  import Footer from "./lib/components/Footer.svelte";
  import Header from "./lib/components/Header.svelte";
  import SetupScreen from "./lib/components/SetupScreen.svelte";
  import UpdateDialog from "./lib/components/UpdateDialog.svelte";

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
  <Header {panel} onDisconnect={() => (panel = null)} />
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
