<script lang="ts">
  import { projectDiff, readLocalFile, readServerFile, uploadCommitDelta } from "../api";
  import { diffManifests } from "../diff";
  import { fileContentAt } from "../snapshotcontent";
  import {
    anonKey,
    createCommit,
    currentBundle,
    deleteCommit,
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
  import MarkdownEditor from "./MarkdownEditor.svelte";

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
  // The state a new commit's delta is measured against (server ⊕ prior commits).
  let accumulatedBase = $state<Manifest>({});
  // The server state the current Deploy's first commit builds on.
  let serverBase = $state<Manifest>({});
  let bundle = $state<DeployBundle | null>(null);
  let commits = $state<CloudCommit[]>([]);
  let loading = $state(true);
  let showFiles = $state(false);
  let showUncommitted = $state(false);
  // A commit in the current Deploy whose file changes are expanded.
  let openCommitId = $state<string | null>(null);
  let openCommitDiff = $state<Diff | null>(null);
  let removingId = $state<string | null>(null);

  let message = $state("");
  let description = $state("");
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

  /** Open the line diff for an uncommitted path (committed state vs local). */
  async function showUncommittedFileDiff(path: string, change: ChangeKind) {
    openDiff = { path, oldText: "", newText: "", loading: true, error: null };
    try {
      const token = await sessionToken();
      const newText = change === "deleted" ? "" : await readLocalFile(config, path);
      let oldText = "";
      if (change !== "added") {
        // The committed version: walk this Deploy's commits for the delta that
        // wrote the file; if none did, it was inherited from the server.
        const storedCommits = commits.filter((c) => c.stored);
        const r = await fileContentAt(storedCommits, 0, path, token, project.id);
        oldText = r.found
          ? r.text
          : await readServerFile(project.panel_id ?? "", project.server_identifier ?? "", path);
      }
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
      serverBase = base;
      const [d, b] = await Promise.all([projectDiff(config, base), currentBundle(project.id)]);
      diff = d;
      bundle = b;
      commits = await listCommits(project.id, b.id);
      openCommitId = null;
      openCommitDiff = null;
      // The base a new commit's delta is measured against: the accumulated
      // committed state = the newest stored commit's manifest, or the server
      // state if nothing is committed yet. Uncommitted changes = local vs it.
      const latest = commits.find((c) => c.stored) ?? null;
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
      const created = await createCommit(project.id, message.trim(), description.trim() || null);
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
      description = "";
      await load();
      onCommitted?.();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      committing = false;
    }
  }

  /** Expand/collapse a current-Deploy commit's file changes (vs its parent). */
  async function toggleCommit(c: CloudCommit) {
    if (openCommitId === c.id) {
      openCommitId = null;
      openCommitDiff = null;
      return;
    }
    openCommitId = c.id;
    openCommitDiff = null;
    try {
      const idx = commits.findIndex((x) => x.id === c.id);
      const parent = idx >= 0 && idx + 1 < commits.length ? commits[idx + 1] : null;
      const [self, base] = await Promise.all([
        getCommitManifest(c.id),
        parent ? getCommitManifest(parent.id) : Promise.resolve(serverBase),
      ]);
      openCommitDiff = diffManifests(base, self);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    }
  }

  /** Open the line diff for one file of a current-Deploy commit. */
  async function showCommitFileDiff(commitId: string, path: string, change: ChangeKind) {
    const idx = commits.findIndex((c) => c.id === commitId);
    if (idx < 0) return;
    openDiff = { path, oldText: "", newText: "", loading: true, error: null };
    try {
      const token = await sessionToken();
      const newRes =
        change === "deleted"
          ? { text: "" }
          : await fileContentAt(commits, idx, path, token, project.id);
      let oldText = "";
      if (change !== "added") {
        const r = await fileContentAt(commits, idx + 1, path, token, project.id);
        oldText = r.found
          ? r.text
          : await readServerFile(project.panel_id ?? "", project.server_identifier ?? "", path);
      }
      openDiff = { path, oldText, newText: newRes.text, loading: false, error: null };
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

  /** Remove the newest commit from the current Deploy (LIFO). */
  async function removeNewest(c: CloudCommit) {
    removingId = c.id;
    error = null;
    try {
      await deleteCommit(c.id);
      await load();
      onCommitted?.();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      removingId = null;
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
            <h4 class="u-title">Uncommitted local changes</h4>
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
        placeholder="Commit name (e.g. Fix login bug)"
        autocomplete="off"
        disabled={committing}
      />
      <MarkdownEditor bind:value={description} rows={3} placeholder="Description (optional) — what changed and why…" />
      <button type="submit" class="primary" disabled={committing || message.trim() === ""}>
        {committing ? "Committing…" : "Commit"}
      </button>
    </form>
    <p class="hint muted">
      A commit records your changes into the current Deploy. Everyone's commits
      ship together when someone presses Deploy.
    </p>

    {#if error}<p class="error small">{error}</p>{/if}

    <div class="bundle">
      <h4>Current Deploy {#if commits.length > 0}<span class="muted">· {commits.length} {commits.length === 1 ? "commit" : "commits"}</span>{/if}</h4>
      {#if commits.length === 0}
        <p class="muted small">No commits yet. Commit your changes to add them to the next deploy.</p>
      {:else}
        <ul class="commits">
          {#each commits as c, i (c.id)}
            <li>
              <div class="c-row">
                <span class="dot" class:pending={!c.stored}></span>
                <button class="c-main" onclick={() => toggleCommit(c)} title="Show file changes">
                  <span class="c-msg">{c.message}</span>
                  <span class="c-meta muted">
                    {actor(c)} · {when(c.created_at)}{#if c.files_count !== null} · {c.files_count} files{/if}
                  </span>
                </button>
                {#if i === 0}
                  <button
                    class="ghost remove"
                    onclick={() => removeNewest(c)}
                    disabled={removingId === c.id}
                    title="Remove this commit from the current Deploy"
                  >
                    {removingId === c.id ? "Removing…" : "Remove"}
                  </button>
                {/if}
              </div>
              {#if openCommitId === c.id}
                {#if c.description}
                  <p class="c-desc muted">{c.description}</p>
                {/if}
                {#if openCommitDiff === null}
                  <p class="muted small pad">Loading changes…</p>
                {:else if openCommitDiff.changes.length === 0}
                  <p class="muted small pad">No file changes.</p>
                {:else}
                  <ul class="files nested">
                    {#each openCommitDiff.changes as change (change.path)}
                      <li>
                        <button class="file {change.change}" onclick={() => showCommitFileDiff(c.id, change.path, change.change)} title="View changes">
                          <span class="sym">{sym[change.change]}</span> {change.path}
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              {/if}
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
    flex-direction: column;
  }

  .c-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    margin-top: 6px;
    background: var(--accent);
  }

  .dot.pending {
    background: var(--warn, #fbbf24);
  }

  .c-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 2px 4px;
    text-align: left;
  }

  .c-main:hover {
    background: var(--surface-2);
  }

  .c-msg {
    font-weight: 600;
    font-size: 13px;
  }

  .c-meta {
    font-size: 12px;
  }

  .remove {
    flex-shrink: 0;
    color: var(--danger);
    font-size: 12px;
  }

  .c-desc {
    margin: 2px 0 6px 19px;
    font-size: 12px;
    white-space: pre-wrap;
  }

  .files.nested {
    margin: 4px 0 6px 19px;
  }

  .pad {
    margin: 4px 0 6px 19px;
  }
</style>
