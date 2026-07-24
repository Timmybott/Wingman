<script lang="ts">
  import {
    getCommitManifest,
    listBundles,
    listCommits,
    listIssues,
    sessionToken,
    type CloudCommit,
    type CloudProject,
    type DeployBundle,
    type Issue,
  } from "../cloud";
  import { diffCounts, diffManifests } from "../diff";
  import { fileContentAt } from "../snapshotcontent";
  import type { ChangeKind, Diff } from "../types";
  import FileDiff from "./FileDiff.svelte";

  let {
    project,
    onRollback,
    onClose,
    focusDeployAt = null,
    canWrite = true,
  }: {
    project: CloudProject;
    /** Restore a past deploy, identified by its bundle id. */
    onRollback: (bundleId: string) => void;
    onClose: () => void;
    /** ISO timestamp of a deploy to open directly (matched to its bundle). */
    focusDeployAt?: string | null;
    /** False for another team's project — rollback is hidden. */
    canWrite?: boolean;
  } = $props();

  let focusConsumed = $state(false);

  type View = "list" | "commit" | "deploy";
  let view = $state<View>("list");
  let tab = $state<"deploys" | "commits">("deploys");

  let bundles = $state<DeployBundle[]>([]);
  let commits = $state<CloudCommit[]>([]);
  let issues = $state<Issue[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Detail state.
  let selected = $state<CloudCommit | DeployBundle | null>(null);
  // Index of the selected commit in `commits` (newest-first), so per-file diffs
  // can walk older commits for a file's real content in the delta model.
  let selectedIndex = $state(-1);
  let detailCommits = $state<CloudCommit[]>([]); // commits in the selected deploy
  let diff = $state<Diff | null>(null);
  let detailLoading = $state(false);
  let armed = $state(false);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  // Per-file diff viewer (commit snapshots).
  let openFileDiff = $state<{
    path: string;
    oldText: string;
    newText: string;
    loading: boolean;
    error: string | null;
  } | null>(null);

  async function showCommitFileDiff(path: string, change: ChangeKind) {
    if (view !== "commit" || selectedIndex < 0) return;
    openFileDiff = { path, oldText: "", newText: "", loading: true, error: null };
    try {
      const token = await sessionToken();
      // New = the file as of this commit (added/modified live in its delta);
      // old = the file as of the parent, found by walking older commits.
      const [newRes, oldRes] = await Promise.all([
        change === "deleted"
          ? Promise.resolve({ text: "" })
          : fileContentAt(commits, selectedIndex, path, token, project.id),
        change === "added"
          ? Promise.resolve({ text: "" })
          : fileContentAt(commits, selectedIndex + 1, path, token, project.id),
      ]);
      openFileDiff = {
        path,
        oldText: oldRes.text,
        newText: newRes.text,
        loading: false,
        error: null,
      };
    } catch (e) {
      openFileDiff = {
        path,
        oldText: "",
        newText: "",
        loading: false,
        error: String(e instanceof Error ? e.message : e),
      };
    }
  }

  const released = $derived(bundles.filter((b) => b.status === "released"));

  async function load() {
    loading = true;
    error = null;
    try {
      [bundles, commits, issues] = await Promise.all([
        listBundles(project.id),
        listCommits(project.id),
        listIssues(project.id).catch(() => [] as Issue[]),
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

  // When opened from a Deploy-history row, jump straight to that deploy. The
  // audit entry and the released bundle are written at almost the same moment,
  // so match on the closest release time (within a few minutes).
  $effect(() => {
    if (focusConsumed || loading || focusDeployAt === null || bundles.length === 0) return;
    focusConsumed = true;
    const target = new Date(focusDeployAt).getTime();
    let best: DeployBundle | null = null;
    let bestDelta = Infinity;
    for (const b of released) {
      const delta = Math.abs(new Date(b.released_at ?? b.created_at).getTime() - target);
      if (delta < bestDelta) {
        bestDelta = delta;
        best = b;
      }
    }
    if (best && bestDelta < 5 * 60 * 1000) {
      tab = "deploys";
      void openDeploy(best);
    }
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
      selectedIndex = idx;
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
      // A deploy introduces no changes of its own — it's exactly the commits it
      // shipped, so we only list those (each links to its own diff).
      detailCommits = await listCommits(project.id, bundle.id);
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

  /** The header Back button: step back within history, or leave it entirely. */
  function headerBack() {
    if (view === "list") onClose();
    else back();
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

  const deployIssues = $derived(
    asBundle ? issues.filter((i) => i.bundle_id === asBundle.id) : [],
  );
  const commitIssues = $derived(
    asCommit ? issues.filter((i) => i.commit_id === asCommit.id) : [],
  );

  function actor(name: string | null): string {
    return name?.trim() || "someone";
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  const sym = { added: "+", modified: "~", deleted: "−" } as const;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (view === "list" ? onClose() : back())} />

{#if openFileDiff}
  <FileDiff
    path={openFileDiff.path}
    oldText={openFileDiff.oldText}
    newText={openFileDiff.newText}
    loading={openFileDiff.loading}
    error={openFileDiff.error}
    onClose={() => (openFileDiff = null)}
  />
{:else}
<div class="history">
  <header>
    <button class="back ghost" onclick={headerBack} title="Back (Esc)">← Back</button>
    <div class="title">
      <h3>{project.name}</h3>
      <span class="muted">history</span>
    </div>
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
      {#if commitIssues.length > 0}
        <h5>Fixes</h5>
        {@render issueList(commitIssues)}
      {/if}
      {@render diffBlock()}
    </div>
  {:else if view === "deploy" && asBundle}
    <div class="detail">
      <h4>{asBundle.message?.trim() || "Deploy"}</h4>
      <p class="d-meta muted">
        {when(asBundle.released_at ?? asBundle.created_at)}
        {#if asBundle.files_count !== null} · {asBundle.files_count} files{/if}
      </p>
      {#if asBundle.description}
        <p class="d-desc">{asBundle.description}</p>
      {/if}
      {#if canWrite}
        <button
          class="rollback"
          class:armed
          onclick={() => rollbackClick(asBundle.id)}
          title="Roll the server back to this deploy's snapshot"
        >
          {armed ? "Sure? Restore this deploy" : "Rollback to this deploy"}
        </button>
      {/if}

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

      {#if deployIssues.length > 0}
        <h5>Issues in this deploy</h5>
        {@render issueList(deployIssues)}
      {/if}

      <p class="muted small note">
        A deploy ships exactly the commits above — open one to see its file
        changes.
      </p>
    </div>
  {/if}
</div>
{/if}

{#snippet issueList(list: Issue[])}
  <ul class="issues">
    {#each list as i (i.id)}
      <li>
        <span class="i-dot {i.status}"></span>
        <span class="i-num muted">#{i.number}</span>
        <span class="i-title">{i.title}</span>
      </li>
    {/each}
  </ul>
{/snippet}

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
          <li>
            <button class="file {ch.change}" onclick={() => showCommitFileDiff(ch.path, ch.change)} title="View changes">
              <span class="fsym">{sym[ch.change]}</span> {ch.path}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
{/snippet}

<style>
  .history {
    /* Full-page in-flow view that fills the main content area, not a drawer. */
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);
    min-height: 380px;
    max-width: 760px;
    margin: 0 auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  header .back {
    flex-shrink: 0;
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

  .d-desc {
    font-size: 13px;
    white-space: pre-wrap;
    line-height: 1.5;
    margin: 0 0 14px;
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

  .file {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 4px;
    padding: 1px 4px;
    font: inherit;
    color: inherit;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.file:hover {
    background: var(--surface-2);
    text-decoration: underline;
  }

  .file.added {
    color: var(--ok, #34d399);
  }

  .file.modified {
    color: var(--warn, #fbbf24);
  }

  .file.deleted {
    color: var(--danger, #f87171);
  }

  .small {
    font-size: 12px;
  }

  .note {
    margin-top: 14px;
  }

  .issues {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 8px;
  }

  .issues li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .i-dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .i-dot.open {
    background: #34d399;
  }

  .i-dot.closed {
    background: #a78bfa;
  }

  .i-num {
    font-size: 12px;
    font-family: ui-monospace, monospace;
  }

  .i-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
