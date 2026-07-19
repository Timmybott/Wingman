<script lang="ts">
  import { appStatus } from "../appStatus.svelte";

  const lastDeploy = $derived(appStatus.lastDeploy);
</script>

<footer>
  {#if lastDeploy}
    <span class="muted">
      Last deploy: {lastDeploy.projectName} · {lastDeploy.at.toLocaleTimeString()} ·
      {lastDeploy.files} files
    </span>
  {:else}
    <!-- M4 puts the active project's git status here
         (e.g. "3 commits since last deploy"). -->
    <span class="muted">No deploy yet this session</span>
  {/if}
  <span class="muted">Notifications: on</span>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 20px;
    background: var(--surface);
    border-top: 1px solid var(--border);
    font-size: 12px;
  }
</style>
