<script lang="ts">
  import { type CloudPanel, type CloudProject } from "../cloud";
  import NewProjectDialog from "./NewProjectDialog.svelte";

  let {
    teamId,
    projects,
    panels,
    loading = false,
    onOpenProject,
    onCreated,
  }: {
    teamId: string;
    projects: CloudProject[];
    panels: CloudPanel[];
    loading?: boolean;
    onOpenProject: (projectId: string) => void;
    onCreated: (project: CloudProject) => void;
  } = $props();

  let error = $state<string | null>(null);
  let showNew = $state(false);

  function panelName(id: string | null): string | null {
    return id ? (panels.find((p) => p.id === id)?.name ?? null) : null;
  }

  function openNew() {
    error = null;
    if (panels.length === 0) {
      error = "Add a panel first (Panels tab) — a project imports one of its servers.";
      return;
    }
    showNew = true;
  }

  function created(project: CloudProject) {
    showNew = false;
    onCreated(project);
  }
</script>

<div class="projects">
  <div class="head">
    <div>
      <h2>Projects</h2>
      <p class="muted">
        Each project imports one of your panel's servers so you can plan, track
        issues, and deploy it. Shared with everyone on the team.
      </p>
    </div>
    <button class="primary" onclick={openNew}>New project</button>
  </div>

  {#if error}<p class="error">{error}</p>{/if}

  {#if loading}
    <p class="muted center">Loading projects…</p>
  {:else if projects.length > 0}
    <ul class="list">
      {#each projects as project (project.id)}
        <li>
          <button class="card" onclick={() => onOpenProject(project.id)}>
            {#if project.logo_url}
              <img class="card-logo" src={project.logo_url} alt={project.name} />
            {:else}
              <span class="card-logo placeholder">{project.name.charAt(0).toUpperCase()}</span>
            {/if}
            <span class="card-body">
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
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty muted">No projects yet. Import a server to get started.</p>
  {/if}
</div>

{#if showNew}
  <NewProjectDialog
    {teamId}
    {panels}
    existing={projects}
    onCreated={created}
    onClose={() => (showNew = false)}
  />
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

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .card {
    display: flex;
    align-items: flex-start;
    gap: 14px;
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

  .card-logo {
    flex-shrink: 0;
    width: 42px;
    height: 42px;
    border-radius: 9px;
    object-fit: cover;
    border: 1px solid var(--border);
  }

  .card-logo.placeholder {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    font-weight: 700;
    font-size: 18px;
  }

  .card-body {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    flex: 1;
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
