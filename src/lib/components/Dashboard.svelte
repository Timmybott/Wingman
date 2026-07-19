<script lang="ts">
  import { onMount } from "svelte";
  import { listServers, serverResources } from "../api";
  import type { Server, ServerStats } from "../types";
  import ServerCard from "./ServerCard.svelte";

  // Polling is the M1 stopgap; M2 replaces it with the console websocket,
  // which streams the same stats live.
  const POLL_INTERVAL_MS = 8000;

  let servers = $state<Server[]>([]);
  let stats = $state<Record<string, ServerStats>>({});
  let error = $state<string | null>(null);
  let loading = $state(true);

  async function refreshStats() {
    await Promise.all(
      servers.map(async (server) => {
        try {
          stats[server.identifier] = await serverResources(server.identifier);
        } catch {
          delete stats[server.identifier];
        }
      }),
    );
  }

  onMount(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    (async () => {
      try {
        servers = await listServers();
        await refreshStats();
        timer = setInterval(refreshStats, POLL_INTERVAL_MS);
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      if (timer) clearInterval(timer);
    };
  });
</script>

{#if loading}
  <p class="muted center">Loading servers…</p>
{:else if error}
  <p class="error center">{error}</p>
{:else if servers.length === 0}
  <p class="muted center">This API key has no servers.</p>
{:else}
  <div class="grid">
    {#each servers as server (server.identifier)}
      <ServerCard {server} stats={stats[server.identifier]} />
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
  }
</style>
