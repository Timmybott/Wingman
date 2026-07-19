<script lang="ts">
  import { onMount } from "svelte";
  import { getPanel } from "./lib/api";
  import type { PanelConfig } from "./lib/types";
  import Dashboard from "./lib/components/Dashboard.svelte";
  import Footer from "./lib/components/Footer.svelte";
  import Header from "./lib/components/Header.svelte";
  import SetupScreen from "./lib/components/SetupScreen.svelte";

  let panel = $state<PanelConfig | null>(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      panel = await getPanel();
    } catch (error) {
      console.error("failed to load panel config:", error);
    } finally {
      loading = false;
    }
  });
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
