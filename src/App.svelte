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

  let panel = $state<PanelConfig | null>(null);
  let loading = $state(true);
  let update = $state<Update | null>(null);
  let updating = $state(false);
  let updateError = $state<string | null>(null);

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
    updating = true;
    updateError = null;
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      updateError = String(e);
      updating = false;
    }
  }
</script>

<div class="shell">
  <Header {panel} onDisconnect={() => (panel = null)} />
  {#if update}
    <div class="update-banner">
      <span>
        Wingman {update.version} is available.
        {#if updateError}<span class="error">{updateError}</span>{/if}
      </span>
      <div class="update-actions">
        <button class="ghost" onclick={() => (update = null)} disabled={updating}>
          Later
        </button>
        <button class="primary" onclick={installUpdate} disabled={updating}>
          {updating ? "Installing…" : "Install & restart"}
        </button>
      </div>
    </div>
  {/if}
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

<style>
  .update-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 20px;
    background: color-mix(in srgb, var(--accent) 18%, var(--surface));
    border-bottom: 1px solid var(--accent);
    font-size: 13px;
  }

  .update-actions {
    display: flex;
    gap: 8px;
  }
</style>
