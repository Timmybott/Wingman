<script lang="ts">
  import { appStatus } from "../appStatus.svelte";

  const lastDeploy = $derived(appStatus.lastDeploy);
  const gitStatus = $derived(appStatus.gitStatus);
</script>

<footer>
  <span class="muted">
    {#if gitStatus}
      {gitStatus.projectName}:
      {#if gitStatus.commitsSince === 0}
        up to date with last deploy
      {:else}
        <span class="pending">
          {gitStatus.commitsSince}
          {gitStatus.commitsSince === 1 ? "commit" : "commits"} since last deploy
        </span>
      {/if}
    {:else if lastDeploy}
      Last deploy: {lastDeploy.projectName} · {lastDeploy.at.toLocaleTimeString()} ·
      {lastDeploy.files} files
    {:else}
      No deploy yet this session
    {/if}
  </span>
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

  .pending {
    color: var(--warn);
  }
</style>
