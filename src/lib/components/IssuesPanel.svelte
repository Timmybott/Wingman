<script lang="ts">
  import { onMount } from "svelte";
  import { createIssue, listIssues, type Issue } from "../cloud";
  import IssueThread from "./IssueThread.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  let {
    projectId,
    onOpenProfile,
  }: {
    projectId: string;
    onOpenProfile?: (userId: string) => void;
  } = $props();

  let issues = $state<Issue[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<"open" | "closed">("open");
  let selectedId = $state<string | null>(null);

  let showForm = $state(false);
  let title = $state("");
  let body = $state("");
  let creating = $state(false);

  const selected = $derived(issues.find((i) => i.id === selectedId) ?? null);
  const openCount = $derived(issues.filter((i) => i.status === "open").length);
  const closedCount = $derived(issues.filter((i) => i.status === "closed").length);
  const visible = $derived(issues.filter((i) => i.status === filter));

  async function load() {
    loading = true;
    error = null;
    try {
      issues = await listIssues(projectId);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (title.trim() === "") return;
    creating = true;
    error = null;
    try {
      await createIssue(projectId, title, body);
      title = "";
      body = "";
      showForm = false;
      filter = "open";
      await load();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      creating = false;
    }
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  }
</script>

{#if selected}
  <IssueThread
    issue={selected}
    {projectId}
    onBack={() => (selectedId = null)}
    onChanged={load}
  />
{:else}
  <div class="issues">
    <div class="bar">
      <div class="filters">
        <button class:active={filter === "open"} onclick={() => (filter = "open")}>
          ● {openCount} Open
        </button>
        <button class:active={filter === "closed"} onclick={() => (filter = "closed")}>
          ✓ {closedCount} Closed
        </button>
      </div>
      {#if !showForm}
        <button class="primary" onclick={() => (showForm = true)}>New issue</button>
      {/if}
    </div>

    {#if error}<p class="error">{error}</p>{/if}

    {#if showForm}
      <form onsubmit={create}>
        <div class="field">
          <label for="i-title">Title</label>
          <input id="i-title" bind:value={title} placeholder="Something to fix or do" autocomplete="off" />
        </div>
        <div class="field">
          <label for="i-body">Description <span class="muted">(optional)</span></label>
          <MarkdownEditor id="i-body" bind:value={body} rows={4} placeholder="Add more detail…" />
        </div>
        <div class="form-actions">
          <button type="button" class="ghost" onclick={() => (showForm = false)} disabled={creating}>Cancel</button>
          <button type="submit" class="primary" disabled={creating || title.trim() === ""}>
            {creating ? "Creating…" : "Create issue"}
          </button>
        </div>
      </form>
    {/if}

    {#if loading}
      <p class="muted center">Loading issues…</p>
    {:else if visible.length === 0}
      <p class="muted center empty">
        {filter === "open" ? "No open issues." : "No closed issues yet."}
      </p>
    {:else}
      <ul class="list">
        {#each visible as issue (issue.id)}
          <li>
            <button class="row" onclick={() => (selectedId = issue.id)}>
              <span class="dot {issue.status}"></span>
              <span class="main">
                <span class="title">{issue.title}</span>
                <span class="meta muted">
                  #{issue.number} · {issue.author_name ?? "someone"} · {when(issue.created_at)}
                </span>
              </span>
              {#if issue.comment_count > 0}
                <span class="comments muted" title="{issue.comment_count} comments">
                  {issue.comment_count} {issue.comment_count === 1 ? "comment" : "comments"}
                </span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
  }

  .filters {
    display: flex;
    gap: 4px;
  }

  .filters button {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    padding: 6px 12px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
  }

  .filters button:hover {
    color: var(--text);
  }

  .filters button.active {
    color: var(--text);
    background: var(--surface-2);
    border-color: var(--border);
  }

  form {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    margin-bottom: 18px;
  }

  .field {
    margin-bottom: 14px;
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }

  .list li + li {
    border-top: 1px solid var(--border);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    text-align: left;
    background: var(--surface);
    border: none;
    border-radius: 0;
    padding: 12px 14px;
  }

  .row:hover {
    background: var(--surface-2);
  }

  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .dot.open {
    background: #34d399;
  }

  .dot.closed {
    background: #a78bfa;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .title {
    font-weight: 600;
    font-size: 14px;
  }

  .meta {
    font-size: 12px;
  }

  .comments {
    flex-shrink: 0;
    font-size: 12px;
  }

  .empty {
    padding: 30px 0;
  }
</style>
