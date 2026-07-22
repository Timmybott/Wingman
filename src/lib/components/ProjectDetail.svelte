<script lang="ts">
  import { deleteProject, updateProject, type CloudPanel, type CloudProject } from "../cloud";

  let {
    project,
    panels,
    onBack,
    onChanged,
    onDeleted,
  }: {
    project: CloudProject;
    panels: CloudPanel[];
    onBack: () => void;
    onChanged: (updated: CloudProject) => void;
    onDeleted: (id: string) => void;
  } = $props();

  let editing = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let confirmingDelete = $state(false);

  // Edit-form buffer, seeded when entering edit mode.
  let name = $state("");
  let description = $state("");
  let panelId = $state<string>("");
  let serverIdentifier = $state("");

  const panelName = $derived(panels.find((p) => p.id === project.panel_id)?.name ?? null);

  function startEdit() {
    name = project.name;
    description = project.description;
    panelId = project.panel_id ?? "";
    serverIdentifier = project.server_identifier ?? "";
    error = null;
    editing = true;
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (name.trim() === "") return;
    busy = true;
    error = null;
    try {
      const updated = await updateProject(project.id, {
        name: name.trim(),
        description: description.trim(),
        panel_id: panelId === "" ? null : panelId,
        server_identifier: serverIdentifier.trim() || null,
      });
      onChanged(updated);
      editing = false;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    busy = true;
    error = null;
    try {
      await deleteProject(project.id);
      onDeleted(project.id);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
      busy = false;
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }
</script>

<div class="detail">
  <button class="back ghost" onclick={onBack}>← All projects</button>

  {#if editing}
    <form onsubmit={save}>
      <div class="field">
        <label for="name">Name</label>
        <input id="name" bind:value={name} autocomplete="off" />
      </div>
      <div class="field">
        <label for="desc">Description</label>
        <textarea id="desc" bind:value={description} rows="6" placeholder="What is this project? Plans, notes, links…"></textarea>
      </div>
      <div class="field">
        <label for="panel">Panel <span class="muted">(optional)</span></label>
        <select id="panel" bind:value={panelId}>
          <option value="">— none —</option>
          {#each panels as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </div>
      <div class="field">
        <label for="server">Server identifier <span class="muted">(optional, e.g. d3aac109)</span></label>
        <input id="server" bind:value={serverIdentifier} spellcheck="false" autocomplete="off" />
      </div>

      {#if error}<p class="error">{error}</p>{/if}

      <div class="actions">
        <button type="button" class="ghost" onclick={() => (editing = false)} disabled={busy}>Cancel</button>
        <button type="submit" class="primary" disabled={busy || name.trim() === ""}>
          {busy ? "Saving…" : "Save"}
        </button>
      </div>
    </form>
  {:else}
    <div class="head">
      <h2>{project.name}</h2>
      <button class="ghost" onclick={startEdit}>Edit</button>
    </div>

    <div class="section">
      <h3>Description</h3>
      {#if project.description.trim() !== ""}
        <p class="description">{project.description}</p>
      {:else}
        <p class="muted">No description yet. Use <strong>Edit</strong> to add plans and notes.</p>
      {/if}
    </div>

    <div class="section meta">
      <div class="meta-item">
        <span class="muted">Panel</span>
        <span>{panelName ?? "— not linked —"}</span>
      </div>
      <div class="meta-item">
        <span class="muted">Server</span>
        <span>{project.server_identifier ?? "— not linked —"}</span>
      </div>
      <div class="meta-item">
        <span class="muted">Created</span>
        <span>{formatDate(project.created_at)}</span>
      </div>
    </div>

    {#if error}<p class="error">{error}</p>{/if}

    <div class="danger-zone">
      {#if confirmingDelete}
        <span class="muted">Delete this project for the whole team?</span>
        <button class="ghost" onclick={() => (confirmingDelete = false)} disabled={busy}>Cancel</button>
        <button class="danger-btn" onclick={remove} disabled={busy}>{busy ? "Deleting…" : "Delete"}</button>
      {:else}
        <button class="ghost danger" onclick={() => (confirmingDelete = true)}>Delete project</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .detail {
    max-width: 720px;
    margin: 24px auto 0;
  }

  .back {
    margin-bottom: 16px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 20px;
  }

  h2 {
    font-size: 22px;
  }

  .section {
    margin-bottom: 22px;
  }

  h3 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .description {
    line-height: 1.6;
    white-space: pre-wrap;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 28px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
  }

  .meta-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }

  .danger-zone {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 28px;
    padding-top: 18px;
    border-top: 1px solid var(--border);
  }

  .danger:hover {
    color: var(--danger, #f87171);
    border-color: var(--danger, #f87171);
  }

  .danger-btn {
    background: var(--danger, #f87171);
    color: #fff;
    border: 1px solid var(--danger, #f87171);
  }

  .field {
    margin-bottom: 14px;
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
  }

  select {
    width: 100%;
  }

  .actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 20px;
  }
</style>
