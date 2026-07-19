<script lang="ts">
  import { removePanel } from "../api";
  import type { PanelConfig } from "../types";

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
    <span class="logo">W</span>
    <h1>Wingman</h1>
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

  .logo {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--accent);
    color: #fff;
    font-weight: 700;
    font-size: 15px;
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
