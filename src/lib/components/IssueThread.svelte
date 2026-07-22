<script lang="ts">
  import { onMount } from "svelte";
  import {
    addComment,
    listComments,
    setIssueStatus,
    type Issue,
    type IssueComment,
    type IssueStatus,
  } from "../cloud";
  import Markdown from "./Markdown.svelte";

  let {
    issue,
    onBack,
    onChanged,
  }: {
    issue: Issue;
    onBack: () => void;
    onChanged: () => void;
  } = $props();

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

  onMount(load);

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
