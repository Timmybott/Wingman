<script lang="ts">
  import {
    getBundleManifest,
    getCommitManifest,
    listBundles,
    listCommits,
    type CloudCommit,
    type CloudProject,
    type DeployBundle,
  } from "../cloud";
  import { diffCounts, diffManifests } from "../diff";
  import type { Diff } from "../types";

  let {
    project,
    onRollback,
    onClose,
  }: {
    project: CloudProject;
    onRollback: (commitId: string) => void;
    onClose: () => void;
  } = $props();

  type View = "list" | "commit" | "deploy";
  let view = $state<View>("list");
  let tab = $state<"deploys" | "commits">("deploys");

  let bundles = $state<DeployBundle[]>([]);
  let commits = $state<CloudCommit[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Detail state.
  let selected = $state<CloudCommit | DeployBundle | null>(null);
  let detailCommits = $state<CloudCommit[]>([]); // commits in the selected deploy
  let diff = $state<Diff | null>(null);
  let detailLoading = $state(false);
  let armed = $state(false);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  const released = $derived(bundles.filter((b) => b.status === "released"));

  async function load() {
    loading = true;
    error = null;
    try {
      [bundles, commits] = await Promise.all([
        listBundles(project.id),
        listCommits(project.id),
      ]);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void project.id;
    void load();
  });

  async function openCommit(commit: CloudCommit) {
    selected = commit;
    view = "commit";
    diff = null;
    armed = false;
    detailLoading = true;
    try {
      // Diff this commit against the previous commit in the project timeline.
      const idx = commits.findIndex((c) => c.id === commit.id);
      const parent = idx >= 0 ? commits[idx + 1] : undefined;
      const [self, base] = await Promise.all([
        getCommitManifest(commit.id),
        parent ? getCommitManifest(parent.id) : Promise.resolve({}),
      ]);
      diff = diffManifests(base, self);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      detailLoading = false;
    }
  }

  async function openDeploy(bundle: DeployBundle) {
    selected = bundle;
    view = "deploy";
    diff = null;
    detailCommits = [];
    detailLoading = true;
    try {
      const idx = released.findIndex((b) => b.id === bundle.id);
      const prev = idx >= 0 ? released[idx + 1] : undefined;
      const [cs, self, base] = await Promise.all([
        listCommits(project.id, bundle.id),
        getBundleManifest(bundle.id),
        prev ? getBundleManifest(prev.id) : Promise.resolve({}),
      ]);
      detailCommits = cs;
      diff = diffManifests(base, self);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      detailLoading = false;
    }
  }

  function back() {
    view = "list";
    selected = null;
    diff = null;
  }

  function rollbackClick(commitId: string) {
    if (!armed) {
      armed = true;
      if (armTimer) clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = false), 4000);
      return;
    }
    if (armTimer) clearTimeout(armTimer);
    armed = false;
    onRollback(commitId);
  }

  const asCommit = $derived(view === "commit" ? (selected as CloudCommit) : null);
  const asBundle = $derived(view === "deploy" ? (selected as DeployBundle) : null);

  function actor(name: string | null): string {
    return name?.trim() || "someone";
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  const sym = { added: "+", modified: "~", deleted: "−" } as const;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (view === "list" ? onClose() : back())} />

<button class="backdrop" aria-label="Close history" onclick={onClose}></button>
<div class="drawer" role="dialog" aria-modal="true" aria-label="Project history">
  <header>
    <div class="title">
      {#if view !== "list"}
        <button class="ghost tiny" onclick={back}>←</button>
      {/if}
      <h3>{project.name}</h3>
      <span class="muted">history</span>
    </div>
    <button class="ghost" onclick={onClose} title="Close (Esc)">✕</button>
  </header>

  {#if loading}
    <p class="muted pad">Loading…</p>
  {:else if error}
    <p class="error pad">{error}</p>
  {:else if view === "list"}
    <nav class="tabs">
      <button class:active={tab === "deploys"} onclick={() => (tab = "deploys")}>
        Deploys {#if released.length}<span class="count">{released.length}</span>{/if}
      </button>
      <button class:active={tab === "commits"} onclick={() => (tab = "commits")}>
        Commits {#if commits.length}<span class="count">{commits.length}</span>{/if}
      </button>
    </nav>

    {#if tab === "deploys"}
      {#if released.length === 0}
        <p class="muted pad">No deploys yet.</p>
      {:else}
        <div class="rows">
          {#each released as b (b.id)}
            <button class="row" onclick={() => openDeploy(b)}>
              <span class="r-title">{b.message?.trim() || "Deploy"}</span>
              <span class="r-meta muted">
                {actor(null)} · {when(b.released_at ?? b.created_at)}{#if b.files_count !== null} · {b.files_count} files{/if}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    {:else if commits.length === 0}
      <p class="muted pad">No commits yet.</p>
    {:else}
      <div class="rows">
        {#each commits as c (c.id)}
          <button class="row" onclick={() => openCommit(c)}>
            <span class="r-title">{c.message}</span>
            <span class="r-meta muted">
              {actor(c.author_name)} · {when(c.created_at)}{#if c.files_count !== null} · {c.files_count} files{/if}
              {#if !c.stored} · <span class="pending">uploading…</span>{/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  {:else if view === "commit" && asCommit}
    <div class="detail">
      <h4>{asCommit.message}</h4>
      <p class="d-meta muted">
        {actor(asCommit.author_name)} · {when(asCommit.created_at)}
        {#if asCommit.files_count !== null} · {asCommit.files_count} files{/if}
      </p>
      <button
        class="rollback"
        class:armed
        onclick={() => rollbackClick(asCommit.id)}
        title="Roll the server back to this commit's snapshot"
      >
        {armed ? "Sure? Deploy this commit" : "Rollback to this commit"}
      </button>
      {@render diffBlock()}
    </div>
  {:else if view === "deploy" && asBundle}
    <div class="detail">
      <h4>{asBundle.message?.trim() || "Deploy"}</h4>
      <p class="d-meta muted">
        {when(asBundle.released_at ?? asBundle.created_at)}
        {#if asBundle.files_count !== null} · {asBundle.files_count} files{/if}
      </p>

      <h5>Commits in this deploy</h5>
      {#if detailCommits.length === 0}
        <p class="muted small">No commits were recorded for this deploy.</p>
      {:else}
        <div class="rows flush">
          {#each detailCommits as c (c.id)}
            <button class="row" onclick={() => openCommit(c)}>
              <span class="r-title">{c.message}</span>
              <span class="r-meta muted">{actor(c.author_name)} · {when(c.created_at)}</span>
            </button>
          {/each}
        </div>
      {/if}

      <h5>Changes on the server</h5>
      {@render diffBlock()}
    </div>
  {/if}
</div>

{#snippet diffBlock()}
  {#if detailLoading}
    <p class="muted small">Computing diff…</p>
  {:else if diff}
    {@const c = diffCounts(diff)}
    {#if diff.changes.length === 0}
      <p class="muted small">No file changes.</p>
    {:else}
      <p class="summary">
        <span class="added">+{c.added}</span>
        <span class="modified">~{c.modified}</span>
        <span class="deleted">−{c.deleted}</span>
      </p>
      <ul class="files">
        {#each diff.changes as ch (ch.path)}
          <li class={ch.change}><span class="fsym">{sym[ch.change]}</span> {ch.path}</li>
        {/each}
      </ul>
    {/if}
  {/if}
{/snippet}

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

  .drawer {
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

  .tiny {
    padding: 2px 8px;
    align-self: center;
  }

  h3 {
    font-size: 14px;
  }

  .pad {
    padding: 12px 16px;
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 0 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 9px 12px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
  }

  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .count {
    font-size: 11px;
    background: var(--surface-2);
    border-radius: 20px;
    padding: 0 7px;
  }

  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 6px 0;
  }

  .rows.flush {
    flex: none;
    overflow: visible;
    padding: 0;
    margin-bottom: 8px;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 0;
    border-bottom: 1px solid var(--surface-2);
    padding: 9px 16px;
  }

  .row:hover {
    background: var(--surface);
  }

  .r-title {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .r-meta {
    font-size: 11.5px;
  }

  .pending {
    color: var(--warn, #fbbf24);
  }

  .detail {
    flex: 1;
    overflow-y: auto;
    padding: 14px 16px;
  }

  h4 {
    font-size: 15px;
    margin-bottom: 4px;
  }

  h5 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin: 18px 0 8px;
  }

  .d-meta {
    font-size: 12px;
    margin-bottom: 12px;
  }

  .rollback {
    font-size: 12px;
    padding: 6px 12px;
    margin-bottom: 8px;
  }

  .rollback.armed {
    background: var(--warn);
    border-color: var(--warn);
    color: #1a1a20;
    font-weight: 600;
  }

  .summary {
    display: flex;
    gap: 10px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    margin: 8px 0;
  }

  .added {
    color: var(--ok, #34d399);
  }

  .modified {
    color: var(--warn, #fbbf24);
  }

  .deleted {
    color: var(--danger, #f87171);
  }

  .files {
    list-style: none;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .fsym {
    display: inline-block;
    width: 12px;
    font-weight: 700;
  }

  .files li.added {
    color: var(--ok, #34d399);
  }

  .files li.modified {
    color: var(--warn, #fbbf24);
  }

  .files li.deleted {
    color: var(--danger, #f87171);
  }

  .small {
    font-size: 12px;
  }
</style>
