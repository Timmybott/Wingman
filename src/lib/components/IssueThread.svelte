<script lang="ts">
  import { onMount } from "svelte";
  import {
    addComment,
    assignIssueCommit,
    listBundles,
    listComments,
    listCommits,
    setIssueStatus,
    type CloudCommit,
    type DeployBundle,
    type Issue,
    type IssueComment,
    type IssueStatus,
  } from "../cloud";
  import Markdown from "./Markdown.svelte";

  let {
    issue,
    projectId,
    onBack,
    onChanged,
  }: {
    issue: Issue;
    projectId: string;
    onBack: () => void;
    onChanged: () => void;
  } = $props();

  let bundles = $state<DeployBundle[]>([]);
  let allCommits = $state<CloudCommit[]>([]);
  // Local echo of the pinned commit for instant feedback after assigning.
  // svelte-ignore state_referenced_locally
  let commitId = $state<string | null>(issue.commit_id);
  let assigning = $state(false);

  const linkedCommit = $derived(allCommits.find((c) => c.id === commitId) ?? null);

  // Chronological deploy number (#1 = oldest) for each bundle id, so commits
  // can be labelled by the deploy they belong to.
  const deployNo = $derived.by(() => {
    const map = new Map<string, number>();
    const total = bundles.length;
    bundles.forEach((b, i) => map.set(b.id, total - i));
    return map;
  });

  const filedBundle = $derived(
    issue.bundle_id ? (bundles.find((b) => b.id === issue.bundle_id) ?? null) : null,
  );

  function bundleLabel(b: DeployBundle): string {
    const no = deployNo.get(b.id);
    const tag = no ? `Deploy #${no}` : "Deploy";
    if (b.status === "released") {
      const when = b.released_at ? new Date(b.released_at).toLocaleDateString() : "";
      return `${tag} · deployed ${when}`.trim();
    }
    if (b.status === "failed") return `${tag} · failed`;
    return `${tag} · current (not deployed yet)`;
  }

  // Every project commit, grouped by the deploy it belongs to (newest deploy
  // first). This lets an issue be pinned to the commit that fixed it no matter
  // which deploy cycle that was — including issues already closed.
  const commitGroups = $derived.by(() => {
    const byBundle = new Map<string, CloudCommit[]>();
    for (const c of allCommits) {
      const key = c.bundle_id ?? "none";
      const list = byBundle.get(key);
      if (list) list.push(c);
      else byBundle.set(key, [c]);
    }
    const groups: { key: string; label: string; commits: CloudCommit[] }[] = [];
    for (const b of bundles) {
      const list = byBundle.get(b.id);
      if (list && list.length > 0) groups.push({ key: b.id, label: bundleLabel(b), commits: list });
    }
    const orphan = byBundle.get("none");
    if (orphan && orphan.length > 0) groups.push({ key: "none", label: "No deploy", commits: orphan });
    return groups;
  });

  async function loadLinks() {
    try {
      const [b, c] = await Promise.all([listBundles(projectId), listCommits(projectId)]);
      bundles = b;
      allCommits = c;
    } catch {
      // linkage is a nicety — never block the thread
    }
  }

  async function assign(event: Event) {
    const value = (event.target as HTMLSelectElement).value || null;
    assigning = true;
    error = null;
    try {
      await assignIssueCommit(issue.id, value);
      commitId = value;
      onChanged();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      assigning = false;
    }
  }

  // Mounted fresh per issue (the list unmounts this on "back"), so seeding the
  // local status from the prop once is intended; toggleStatus keeps it in sync.
  // svelte-ignore state_referenced_locally
  let status = $state<IssueStatus>(issue.status);
  let comments = $state<IssueComment[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let newComment = $state("");
  let posting = $state(false);
  let toggling = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      comments = await listComments(issue.id);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void load();
    void loadLinks();
  });

  async function toggleStatus() {
    const next: IssueStatus = status === "open" ? "closed" : "open";
    toggling = true;
    error = null;
    try {
      await setIssueStatus(issue.id, next);
      status = next;
      onChanged();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      toggling = false;
    }
  }

  async function post(event: SubmitEvent) {
    event.preventDefault();
    if (newComment.trim() === "") return;
    posting = true;
    error = null;
    try {
      await addComment(issue.id, newComment);
      newComment = "";
      await load();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      posting = false;
    }
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<div class="thread">
  <button class="back ghost" onclick={onBack}>← All issues</button>

  <div class="head">
    <h2>
      {issue.title}
      <span class="muted number">#{issue.number}</span>
    </h2>
    <span class="status status-{status}">{status === "open" ? "● Open" : "✓ Closed"}</span>
  </div>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="link-box">
    {#if filedBundle}
      <div class="link-row">
        <span class="l-label muted">Filed</span>
        <span>Against {bundleLabel(filedBundle)}</span>
      </div>
    {/if}
    <div class="link-row">
      <span class="l-label muted">Fixed in</span>
      {#if allCommits.length > 0}
        <select value={commitId ?? ""} onchange={assign} disabled={assigning}>
          <option value="">— not linked to a commit —</option>
          {#each commitGroups as g (g.key)}
            <optgroup label={g.label}>
              {#each g.commits as c (c.id)}
                <option value={c.id}>{c.message}</option>
              {/each}
            </optgroup>
          {/each}
        </select>
      {:else}
        <span class="muted">no commits in this project yet</span>
      {/if}
    </div>
    {#if linkedCommit}
      <div class="link-row">
        <span class="l-label muted">Pinned</span>
        <span class="pinned">
          ✓ {linkedCommit.message}{#if linkedCommit.bundle_id && deployNo.get(linkedCommit.bundle_id)}
            <span class="muted"> · Deploy #{deployNo.get(linkedCommit.bundle_id)}</span>
          {/if}
        </span>
      </div>
    {/if}
  </div>

  <article class="comment original">
    <div class="c-head">
      <span class="author">{issue.author_name ?? "someone"}</span>
      <span class="muted">opened this · {when(issue.created_at)}</span>
    </div>
    {#if issue.body.trim() !== ""}
      <Markdown source={issue.body} />
    {:else}
      <p class="muted">No description.</p>
    {/if}
  </article>

  {#if loading}
    <p class="muted center">Loading comments…</p>
  {:else}
    {#each comments as c (c.id)}
      <article class="comment">
        <div class="c-head">
          <span class="author">{c.author_name ?? "someone"}</span>
          <span class="muted">{when(c.created_at)}</span>
        </div>
        <Markdown source={c.body} />
      </article>
    {/each}
  {/if}

  <form onsubmit={post}>
    <textarea bind:value={newComment} rows="3" placeholder="Leave a comment…"></textarea>
    <div class="form-actions">
      <button
        type="button"
        class="ghost toggle"
        class:reopen={status === "closed"}
        onclick={toggleStatus}
        disabled={toggling}
      >
        {status === "open" ? "Close issue" : "Reopen issue"}
      </button>
      <button type="submit" class="primary" disabled={posting || newComment.trim() === ""}>
        {posting ? "Posting…" : "Comment"}
      </button>
    </div>
  </form>
</div>

<style>
  .thread {
    max-width: 760px;
  }

  .back {
    margin-bottom: 14px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
  }

  h2 {
    font-size: 20px;
    line-height: 1.3;
  }

  .number {
    font-weight: 400;
  }

  .status {
    flex-shrink: 0;
    font-size: 12px;
    font-weight: 600;
    border-radius: 20px;
    padding: 4px 12px;
    white-space: nowrap;
  }

  .status-open {
    background: #10b98122;
    color: #34d399;
  }

  .status-closed {
    background: #8b5cf622;
    color: #a78bfa;
  }

  .link-box {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 16px;
    margin-bottom: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .link-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    flex-wrap: wrap;
  }

  .l-label {
    flex-shrink: 0;
    width: 60px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .link-row select {
    flex: 1;
    min-width: 180px;
  }

  .pinned {
    color: #34d399;
    font-weight: 600;
  }

  .comment {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 10px;
  }

  .comment.original {
    border-color: var(--accent);
  }

  .c-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 8px;
    font-size: 13px;
  }

  .author {
    font-weight: 600;
  }

  form {
    margin-top: 16px;
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
  }

  .form-actions {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    margin-top: 10px;
  }

  .toggle.reopen {
    color: #34d399;
  }
</style>
