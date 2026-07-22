<script lang="ts">
  import { onMount } from "svelte";
  import {
    createProject,
    listMembers,
    listPanels,
    listProjects,
    type CloudPanel,
    type CloudProject,
    type TeamMember,
  } from "../cloud";
  import ProjectDetail from "./ProjectDetail.svelte";

  let { teamId }: { teamId: string } = $props();

  let projects = $state<CloudProject[]>([]);
  let panels = $state<CloudPanel[]>([]);
  let members = $state<TeamMember[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let selectedId = $state<string | null>(null);

  // Create form
  let showForm = $state(false);
  let newName = $state("");
  let newDescription = $state("");
  let newPanelId = $state<string>("");
  let creating = $state(false);

  const selected = $derived(projects.find((p) => p.id === selectedId) ?? null);

  async function load() {
    loading = true;
    error = null;
    try {
      [projects, panels, members] = await Promise.all([
        listProjects(teamId),
        listPanels(teamId),
        listMembers(teamId),
      ]);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function panelName(id: string | null): string | null {
    return id ? (panels.find((p) => p.id === id)?.name ?? null) : null;
  }

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (newName.trim() === "") return;
    creating = true;
    error = null;
    try {
      const created = await createProject(teamId, {
        name: newName,
        description: newDescription,
        panel_id: newPanelId === "" ? null : newPanelId,
      });
      projects.push(created);
      newName = "";
      newDescription = "";
      newPanelId = "";
      showForm = false;
      selectedId = created.id;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      creating = false;
    }
  }

  function onChanged(updated: CloudProject) {
    const i = projects.findIndex((p) => p.id === updated.id);
    if (i >= 0) projects[i] = updated;
  }

  function onDeleted(id: string) {
    projects = projects.filter((p) => p.id !== id);
    selectedId = null;
  }
</script>

{#if loading}
  <p class="muted center">Loading projects…</p>
{:else if selected}
  <ProjectDetail
    project={selected}
    {panels}
    {members}
    onBack={() => (selectedId = null)}
    {onChanged}
    {onDeleted}
  />
{:else}
  <div class="projects">
    <div class="head">
      <div>
        <h2>Projects</h2>
        <p class="muted">
          Everything your team works on. Describe it, plan it, and — once a panel
          is connected — deploy it. Shared with everyone on the team.
        </p>
      </div>
      {#if !showForm}
        <button class="primary" onclick={() => (showForm = true)}>New project</button>
      {/if}
    </div>

    {#if error}<p class="error">{error}</p>{/if}

    {#if showForm}
      <form onsubmit={create}>
        <div class="field">
          <label for="pname">Name</label>
          <input id="pname" bind:value={newName} placeholder="My Discord bot" autocomplete="off" />
        </div>
        <div class="field">
          <label for="pdesc">Description <span class="muted">(optional)</span></label>
          <textarea id="pdesc" bind:value={newDescription} rows="3" placeholder="What is it? What's the plan?"></textarea>
        </div>
        <div class="field">
          <label for="ppanel">Panel <span class="muted">(optional — you can link it later)</span></label>
          <select id="ppanel" bind:value={newPanelId}>
            <option value="">— none —</option>
            {#each panels as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <div class="actions">
          <button type="button" class="ghost" onclick={() => (showForm = false)} disabled={creating}>Cancel</button>
          <button type="submit" class="primary" disabled={creating || newName.trim() === ""}>
            {creating ? "Creating…" : "Create project"}
          </button>
        </div>
      </form>
    {/if}

    {#if projects.length > 0}
      <ul class="list">
        {#each projects as project (project.id)}
          <li>
            <button class="card" onclick={() => (selectedId = project.id)}>
              <span class="name">{project.name}</span>
              {#if project.description.trim() !== ""}
                <span class="muted desc">{project.description}</span>
              {:else}
                <span class="muted desc empty">No description yet</span>
              {/if}
              <span class="tags">
                {#if panelName(project.panel_id)}
                  <span class="tag">{panelName(project.panel_id)}</span>
                {/if}
                {#if project.server_identifier}
                  <span class="tag mono">{project.server_identifier}</span>
                {/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {:else if !showForm}
      <p class="empty muted">No projects yet. Create your first one.</p>
    {/if}
  </div>
{/if}

<style>
  .projects {
    max-width: 720px;
    margin: 24px auto 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  .head .primary {
    flex-shrink: 0;
  }

  h2 {
    font-size: 20px;
    margin-bottom: 6px;
  }

  p {
    margin: 0;
    line-height: 1.5;
  }

  form {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
    margin-bottom: 20px;
  }

  .field {
    margin-bottom: 14px;
  }

  textarea,
  select {
    width: 100%;
    font: inherit;
  }

  textarea {
    resize: vertical;
  }

  .actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 18px;
  }

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 100%;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
  }

  .card:hover {
    border-color: var(--accent);
  }

  .name {
    font-weight: 600;
    font-size: 15px;
  }

  .desc {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desc.empty {
    font-style: italic;
    opacity: 0.7;
  }

  .tags {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }

  .tag {
    font-size: 11px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 2px 9px;
  }

  .mono {
    font-family: ui-monospace, monospace;
  }

  .empty {
    text-align: center;
    padding: 28px 0;
  }
</style>
