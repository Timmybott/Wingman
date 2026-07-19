<script lang="ts">
  import { commitProject, deployStatus, projectHistory, repoStatus } from "../api";
  import type { CommitInfo, ProjectConfig, RepoStatus } from "../types";

  let {
    project,
    onRollback,
    onChanged,
    onClose,
  }: {
    project: ProjectConfig;
    onRollback: (commitId: string) => void;
    /** Fired after a commit so the dashboard can refresh the footer. */
    onChanged: () => void;
    onClose: () => void;
  } = $props();

  let status = $state<RepoStatus | null>(null);
  let history = $state<CommitInfo[]>([]);
  let deployedCommit = $state<string | null>(null);
  let message = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let armedRollback = $state<string | null>(null);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  async function load() {
    error = null;
    try {
      const [st, log, ds] = await Promise.all([
        repoStatus(project.id),
        projectHistory(project.id, 50),
        deployStatus(project.id),
      ]);
      status = st;
      history = log;
      deployedCommit = ds.last_deploy?.commit ?? null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void project.id;
    loading = true;
    void load();
  });

  async function commit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    try {
      await commitProject(project.id, message);
      message = "";
      await load();
      onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Rollback re-deploys an old state — arm first, confirm second.
  function rollbackClick(commitId: string) {
    if (armedRollback !== commitId) {
      armedRollback = commitId;
      if (armTimer) clearTimeout(armTimer);
      armTimer = setTimeout(() => (armedRollback = null), 4000);
      return;
    }
    if (armTimer) clearTimeout(armTimer);
    armedRollback = null;
    onRollback(commitId);
  }

  function formatTime(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<button class="backdrop" aria-label="Close history" onclick={onClose}></button>
<div class="history" role="dialog" aria-modal="true" aria-label="Project history">
  <header>
    <div class="title">
      <h3>{project.name}</h3>
      <span class="muted">history</span>
    </div>
    <button class="ghost" onclick={onClose} title="Close (Esc)">✕</button>
  </header>

  {#if loading}
    <p class="muted pad">Loading…</p>
  {:else}
    <form class="commit-box" onsubmit={commit}>
      {#if status?.dirty}
        <p class="changes">
          {status.changed.length}
          {status.changed.length === 1 ? "change" : "changes"}:
          <span class="muted">
            {status.changed
              .slice(0, 3)
              .map((c) => `${c.path} (${c.kind})`)
              .join(", ")}{status.changed.length > 3 ? ", …" : ""}
          </span>
        </p>
        <div class="commit-row">
          <input
            bind:value={message}
            placeholder="Commit message"
            autocomplete="off"
            disabled={busy}
          />
          <button type="submit" class="primary" disabled={busy}>Commit</button>
        </div>
      {:else}
        <p class="muted">Working tree clean — nothing to commit.</p>
      {/if}
    </form>

    {#if error}
      <p class="error pad">{error}</p>
    {/if}

    <div class="commits">
      {#if history.length === 0}
        <p class="muted pad">No commits yet. Commits are created here or automatically on deploy.</p>
      {/if}
      {#each history as commit (commit.id)}
        <div class="commit">
          <div class="commit-main">
            <span class="summary">{commit.summary}</span>
            <span class="meta muted">
              <code>{commit.short_id}</code>
              · {commit.author} · {formatTime(commit.timestamp)}
              {#if commit.id === deployedCommit}
                · <span class="deployed">deployed</span>
              {/if}
            </span>
          </div>
          {#if commit.id !== deployedCommit}
            <button
              class="rollback"
              class:armed={armedRollback === commit.id}
              onclick={() => rollbackClick(commit.id)}
              title="Deploy this commit"
            >
              {armedRollback === commit.id ? "Sure?" : "Rollback"}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    border: none;
    border-radius: 0;
    cursor: default;
    z-index: 10;
  }

  .history {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(560px, 92vw);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border-left: 1px solid var(--border);
    z-index: 11;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  h3 {
    font-size: 14px;
  }

  .pad {
    padding: 12px 16px;
  }

  .commit-box {
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .changes {
    margin: 0 0 8px;
    font-size: 12px;
  }

  .commit-row {
    display: flex;
    gap: 8px;
  }

  .commits {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .commit {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--surface-2);
  }

  .commit-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .summary {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    font-size: 11.5px;
  }

  code {
    background: var(--surface-2);
    border-radius: 4px;
    padding: 0 4px;
  }

  .deployed {
    color: var(--ok);
    font-weight: 600;
  }

  .rollback {
    font-size: 12px;
    padding: 5px 10px;
    white-space: nowrap;
  }

  .rollback.armed {
    background: var(--warn);
    border-color: var(--warn);
    color: #1a1a20;
    font-weight: 600;
  }
</style>
