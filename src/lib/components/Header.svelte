<script lang="ts">
  import { removePanel } from "../api";
  import type { PanelConfig } from "../types";
  import Logo from "./Logo.svelte";

  let { panel, onDisconnect }: { panel: PanelConfig | null; onDisconnect: () => void } =
    $props();

  let busy = $state(false);

  async function disconnect() {
    busy = true;
    try {
      await removePanel();
      onDisconnect();
    } catch (error) {
      console.error("failed to disconnect:", error);
    } finally {
      busy = false;
    }
  }
</script>

<header>
  <div class="brand">
    <Logo size={26} />
    <h1>Feather</h1>
  </div>
  <div class="connection">
    {#if panel}
      <span class="dot online"></span>
      <span class="muted" title={panel.base_url}>{panel.name}</span>
      <button class="ghost" onclick={disconnect} disabled={busy}>Disconnect</button>
    {:else}
      <span class="dot unknown"></span>
      <span class="muted">Not connected</span>
    {/if}
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  h1 {
    font-size: 15px;
  }

  .connection {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
