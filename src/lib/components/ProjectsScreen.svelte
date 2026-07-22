<script lang="ts">
  import { onMount } from "svelte";
  import {
    listMembers,
    listPanels,
    listProjects,
    type CloudPanel,
    type CloudProject,
    type TeamMember,
  } from "../cloud";
  import NewProjectDialog from "./NewProjectDialog.svelte";
  import ProjectDetail from "./ProjectDetail.svelte";

  let {
    teamId,
    teamName,
    openProjectId = null,
    onConsumedFocus,
    onOpenServer,
    onOpenTeam,
    onOpenProfile,
  }: {
    teamId: string;
    teamName: string;
    openProjectId?: string | null;
    onConsumedFocus?: () => void;
    onOpenServer: (panelId: string, identifier: string) => void;
    onOpenTeam: () => void;
    onOpenProfile: (userId: string) => void;
  } = $props();

  let projects = $state<CloudProject[]>([]);
  let panels = $state<CloudPanel[]>([]);
  let members = $state<TeamMember[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let selectedId = $state<string | null>(null);
  let showNew = $state(false);

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

  // Open a project requested from elsewhere (e.g. a server tile in Panels),
  // once it's loaded.
  $effect(() => {
    if (openProjectId && projects.some((p) => p.id === openProjectId)) {
      selectedId = openProjectId;
      onConsumedFocus?.();
    }
  });

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

  function onCreated(project: CloudProject) {
    projects.push(project);
    showNew = false;
    selectedId = project.id;
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
    {teamName}
    onBack={() => (selectedId = null)}
    {onChanged}
    {onDeleted}
    {onOpenServer}
    {onOpenTeam}
    {onOpenProfile}
  />
{:else}
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

    {#if projects.length > 0}
      <ul class="list">
        {#each projects as project (project.id)}
          <li>
            <button class="card" onclick={() => (selectedId = project.id)}>
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
      {onCreated}
      onClose={() => (showNew = false)}
    />
  {/if}
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
