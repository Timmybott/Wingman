<script lang="ts">
  import {
    projectDiff,
    readLocalFile,
    readServerFile,
    snapshotFile,
    uploadCommitDelta,
  } from "../api";
  import {
    anonKey,
    createCommit,
    currentBundle,
    finalizeCommit,
    getCommitManifest,
    listCommits,
    serverManifest,
    sessionToken,
    storageAvailable,
    STORAGE_ENDPOINT,
    type CloudCommit,
    type CloudProject,
    type DeployBundle,
  } from "../cloud";
  import type { ChangeKind, Diff, Manifest, ProjectConfig } from "../types";
  import FileDiff from "./FileDiff.svelte";

  let {
    project,
    config,
    refresh = 0,
    onCommitted,
  }: {
    project: CloudProject;
    config: ProjectConfig;
    refresh?: number;
    onCommitted?: () => void;
  } = $props();

  let storageOk = $state<boolean | null>(null);
  let diff = $state<Diff | null>(null);
  // Local changes not yet committed (local vs the newest stored commit in this
  // Deploy). Null when there are no commits yet — then "changes since last
  // deploy" already covers everything, so we don't show a second panel.
  let uncommittedDiff = $state<Diff | null>(null);
  let latestStoredCommitId = $state<string | null>(null);
  // The state a new commit's delta is measured against (server ⊕ prior commits).
  let accumulatedBase = $state<Manifest>({});
  let bundle = $state<DeployBundle | null>(null);
  let commits = $state<CloudCommit[]>([]);
  let loading = $state(true);
  let showFiles = $state(false);
  let showUncommitted = $state(false);

  let message = $state("");
  let committing = $state(false);
  let error = $state<string | null>(null);

  // Per-file diff viewer state.
  let openDiff = $state<{
    path: string;
    oldText: string;
    newText: string;
    loading: boolean;
    error: string | null;
  } | null>(null);

  /** Open the line-level diff for one changed path (server version vs local). */
  async function showFileDiff(path: string, change: ChangeKind) {
    openDiff = { path, oldText: "", newText: "", loading: true, error: null };
    try {
      const [oldText, newText] = await Promise.all([
        change === "added"
          ? Promise.resolve("")
          : readServerFile(project.panel_id ?? "", project.server_identifier ?? "", path),
        change === "deleted" ? Promise.resolve("") : readLocalFile(config, path),
      ]);
      openDiff = { path, oldText, newText, loading: false, error: null };
    } catch (e) {
      openDiff = {
        path,
        oldText: "",
        newText: "",
        loading: false,
        error: String(e instanceof Error ? e.message : e),
      };
    }
  }

  /** Open the line diff for an uncommitted path (last commit's snapshot vs local). */
  async function showUncommittedFileDiff(path: string, change: ChangeKind) {
    if (!latestStoredCommitId) return;
    openDiff = { path, oldText: "", newText: "", loading: true, error: null };
    try {
      const token = await sessionToken();
      const [oldText, newText] = await Promise.all([
        change === "added"
          ? Promise.resolve("")
          : snapshotFile(STORAGE_ENDPOINT, token, anonKey, project.id, latestStoredCommitId, path),
        change === "deleted" ? Promise.resolve("") : readLocalFile(config, path),
      ]);
      openDiff = { path, oldText, newText, loading: false, error: null };
    } catch (e) {
      openDiff = {
        path,
        oldText: "",
        newText: "",
        loading: false,
        error: String(e instanceof Error ? e.message : e),
      };
    }
  }

  function tally(d: Diff | null): { a: number; m: number; d: number } {
    let a = 0;
    let m = 0;
    let del = 0;
    for (const c of d?.changes ?? []) {
      if (c.change === "added") a++;
      else if (c.change === "modified") m++;
      else del++;
    }
    return { a, m, d: del };
  }

  const counts = $derived(tally(diff));
  const uncommittedCounts = $derived(tally(uncommittedDiff));
  const hasChanges = $derived((diff?.changes.length ?? 0) > 0);
  const hasUncommitted = $derived((uncommittedDiff?.changes.length ?? 0) > 0);

  // Reload whenever the parent bumps `refresh` (e.g. after a deploy).
  $effect(() => {
    void refresh;
    void load();
  });

  async function load() {
    loading = true;
    error = null;
    try {
      storageOk = await storageAvailable();
      if (!storageOk) return;
      const base = await serverManifest(project.id);
      const [d, b] = await Promise.all([projectDiff(config, base), currentBundle(project.id)]);
      diff = d;
      bundle = b;
      commits = await listCommits(project.id, b.id);
      // The base a new commit's delta is measured against: the accumulated
      // committed state = the newest stored commit's manifest, or the server
      // state if nothing is committed yet. Uncommitted changes = local vs it.
      const latest = commits.find((c) => c.stored) ?? null;
      latestStoredCommitId = latest?.id ?? null;
      if (latest) {
        accumulatedBase = await getCommitManifest(latest.id);
        uncommittedDiff = await projectDiff(config, accumulatedBase);
      } else {
        accumulatedBase = base;
        uncommittedDiff = null;
      }
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  async function commit(event: SubmitEvent) {
    event.preventDefault();
    if (message.trim() === "") return;
    committing = true;
    error = null;
    try {
      const created = await createCommit(project.id, message.trim());
      const token = await sessionToken();
      // Store only this commit's delta (vs the accumulated committed state); the
      // returned manifest is the full resulting tree, recorded so a deploy can
      // apply the whole bundle.
      const up = await uploadCommitDelta(
        config,
        accumulatedBase,
        STORAGE_ENDPOINT,
        token,
        anonKey,
        project.id,
        created.id,
      );
      await finalizeCommit(created.id, up.files, up.manifest);
      message = "";
      await load();
      onCommitted?.();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      committing = false;
    }
  }

  function actor(c: CloudCommit): string {
    return c.author_name?.trim() || "someone";
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const sym = { added: "+", modified: "~", deleted: "−" } as const;
</script>

<div class="card">
  {#if loading && storageOk === null}
    <p class="muted small">Checking cloud storage…</p>
  {:else if storageOk === false}
    <p class="muted small">
      Cloud commits are unavailable — the storage backend isn't configured yet.
      Deploys still work from the button below.
    </p>
  {:else}
    <div class="head">
      <h3>Changes since last deploy</h3>
      {#if hasChanges}
        <span class="summary">
          <span class="added">+{counts.a}</span>
          <span class="modified">~{counts.m}</span>
          <span class="deleted">−{counts.d}</span>
          <button class="ghost tiny" onclick={() => (showFiles = !showFiles)}>
            {showFiles ? "Hide" : "Show"} files
          </button>
        </span>
      {/if}
    </div>

    {#if !hasChanges}
      <p class="muted small">Your local folder matches the deployed server state.</p>
    {:else if showFiles}
      <ul class="files">
        {#each diff?.changes ?? [] as change (change.path)}
          <li>
            <button class="file {change.change}" onclick={() => showFileDiff(change.path, change.change)} title="View changes">
              <span class="sym">{sym[change.change]}</span> {change.path}
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if uncommittedDiff !== null}
      <div class="uncommitted" class:dirty={hasUncommitted}>
        {#if hasUncommitted}
          <div class="head">
            <h4 class="u-title">⚠ Uncommitted local changes</h4>
            <span class="summary">
              <span class="added">+{uncommittedCounts.a}</span>
              <span class="modified">~{uncommittedCounts.m}</span>
              <span class="deleted">−{uncommittedCounts.d}</span>
              <button class="ghost tiny" onclick={() => (showUncommitted = !showUncommitted)}>
                {showUncommitted ? "Hide" : "Show"} files
              </button>
            </span>
          </div>
          <p class="hint muted">
            These edits are newer than your last commit — commit them to include
            them in the next deploy.
          </p>
          {#if showUncommitted}
            <ul class="files">
              {#each uncommittedDiff.changes as change (change.path)}
                <li>
                  <button class="file {change.change}" onclick={() => showUncommittedFileDiff(change.path, change.change)} title="View changes">
                    <span class="sym">{sym[change.change]}</span> {change.path}
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {:else}
          <p class="muted small">✓ All local changes are committed to the current Deploy.</p>
        {/if}
      </div>
    {/if}

    <form class="commit" onsubmit={commit}>
      <input
        bind:value={message}
        placeholder="Commit message (e.g. Commit v2.4.0)"
        autocomplete="off"
        disabled={committing}
      />
      <button type="submit" class="primary" disabled={committing || message.trim() === ""}>
        {committing ? "Committing…" : "Commit"}
      </button>
    </form>
    <p class="hint muted">
      A commit snapshots your local folder into the current Deploy. Everyone's
      commits ship together when someone presses Deploy.
    </p>

    {#if error}<p class="error small">{error}</p>{/if}

    <div class="bundle">
      <h4>Current Deploy {#if commits.length > 0}<span class="muted">· {commits.length} {commits.length === 1 ? "commit" : "commits"}</span>{/if}</h4>
      {#if commits.length === 0}
        <p class="muted small">No commits yet. Commit your changes to add them to the next deploy.</p>
      {:else}
        <ul class="commits">
          {#each commits as c (c.id)}
            <li>
              <span class="dot" class:pending={!c.stored}></span>
              <div class="c-main">
                <span class="c-msg">{c.message}</span>
                <span class="c-meta muted">
                  {actor(c)} · {when(c.created_at)}{#if c.files_count !== null} · {c.files_count} files{/if}
                </span>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

{#if openDiff}
  <FileDiff
    path={openDiff.path}
    oldText={openDiff.oldText}
    newText={openDiff.newText}
    loading={openDiff.loading}
    error={openDiff.error}
    onClose={() => (openDiff = null)}
  />
{/if}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    margin-bottom: 22px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 8px;
  }

  h3 {
    font-size: 13px;
  }

  h4 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin-bottom: 10px;
  }

  .summary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-family: ui-monospace, monospace;
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

  .tiny {
    padding: 1px 8px;
    font-size: 11px;
    font-family: inherit;
  }

  .files {
    list-style: none;
    margin: 6px 0 12px;
    max-height: 180px;
    overflow-y: auto;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
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

  .file:hover {
    background: var(--surface-2);
    text-decoration: underline;
  }

  .file .sym {
    display: inline-block;
    width: 12px;
    font-weight: 700;
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

  .uncommitted {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .uncommitted.dirty .u-title {
    color: var(--warn, #fbbf24);
  }

  .u-title {
    font-size: 12px;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text);
    margin-bottom: 0;
  }

  .commit {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .commit input {
    flex: 1;
  }

  .hint {
    font-size: 12px;
    margin: 8px 0 0;
    line-height: 1.4;
  }

  .small {
    font-size: 12px;
    margin: 0;
  }

  .bundle {
    margin-top: 16px;
    border-top: 1px solid var(--border);
    padding-top: 14px;
  }

  .commits {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .commits li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    margin-top: 4px;
    background: var(--accent);
  }

  .dot.pending {
    background: var(--warn, #fbbf24);
  }

  .c-main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .c-msg {
    font-weight: 600;
    font-size: 13px;
  }

  .c-meta {
    font-size: 12px;
  }
</style>
