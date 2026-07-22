<script lang="ts">
  import {
    deleteProject,
    updateProject,
    type CloudPanel,
    type CloudProject,
    type PostDeploy,
    type TeamMember,
  } from "../cloud";

  let {
    project,
    panels,
    members,
    onBack,
    onChanged,
    onDeleted,
  }: {
    project: CloudProject;
    panels: CloudPanel[];
    members: TeamMember[];
    onBack: () => void;
    onChanged: (updated: CloudProject) => void;
    onDeleted: (id: string) => void;
  } = $props();

  type Tab = "overview" | "settings";
  let tab = $state<Tab>("overview");

  let error = $state<string | null>(null);

  // Overview: quick inline description edit.
  let editingDescription = $state(false);
  let descriptionDraft = $state("");
  let savingDescription = $state(false);

  // Settings form buffer.
  let name = $state("");
  let description = $state("");
  let panelId = $state<string>("");
  let serverIdentifier = $state("");
  let targetDir = $state("");
  let buildCommand = $state("");
  let postDeploy = $state<PostDeploy>("restart");
  let autoBackup = $state(true);
  let savingSettings = $state(false);

  let confirmingDelete = $state(false);
  let deleting = $state(false);

  const panelName = $derived(panels.find((p) => p.id === project.panel_id)?.name ?? null);
  const creator = $derived(
    project.created_by ? members.find((m) => m.user_id === project.created_by) : undefined,
  );
  const creatorName = $derived(creator?.display_name?.trim() || creator?.username || null);

  function seedSettings() {
    name = project.name;
    description = project.description;
    panelId = project.panel_id ?? "";
    serverIdentifier = project.server_identifier ?? "";
    targetDir = project.target_dir;
    buildCommand = project.build_command ?? "";
    postDeploy = project.post_deploy;
    autoBackup = project.auto_backup;
  }

  function openSettings() {
    seedSettings();
    error = null;
    tab = "settings";
  }

  function startDescriptionEdit() {
    descriptionDraft = project.description;
    error = null;
    editingDescription = true;
  }

  async function saveDescription() {
    savingDescription = true;
    error = null;
    try {
      const updated = await updateProject(project.id, { description: descriptionDraft.trim() });
      onChanged(updated);
      editingDescription = false;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      savingDescription = false;
    }
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    if (name.trim() === "") return;
    savingSettings = true;
    error = null;
    try {
      const updated = await updateProject(project.id, {
        name: name.trim(),
        description: description.trim(),
        panel_id: panelId === "" ? null : panelId,
        server_identifier: serverIdentifier.trim() || null,
        target_dir: targetDir.trim(),
        build_command: buildCommand.trim() === "" ? null : buildCommand.trim(),
        post_deploy: postDeploy,
        auto_backup: autoBackup,
      });
      onChanged(updated);
      tab = "overview";
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      savingSettings = false;
    }
  }

  async function remove() {
    deleting = true;
    error = null;
    try {
      await deleteProject(project.id);
      onDeleted(project.id);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
      deleting = false;
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }
</script>

<div class="detail">
  <button class="back ghost" onclick={onBack}>← All projects</button>

  <header class="project-head">
    <h1>{project.name}</h1>
    <div class="subline">
      {#if panelName}
        <span class="tag">{panelName}</span>
      {/if}
      {#if project.server_identifier}
        <span class="tag mono">{project.server_identifier}</span>
      {:else}
        <span class="muted">Not linked to a server yet</span>
      {/if}
    </div>
  </header>

  <nav class="subtabs">
    <button class:active={tab === "overview"} onclick={() => (tab = "overview")}>Overview</button>
    <button class:active={tab === "settings"} onclick={openSettings}>Settings</button>
    <span class="soon" title="Coming in the next milestones">Issues · Deploys · Planning soon</span>
  </nav>

  {#if error}<p class="error">{error}</p>{/if}

  {#if tab === "overview"}
    <div class="overview">
      <div class="main">
        <div class="card">
          <div class="card-head">
            <h2>About</h2>
            {#if !editingDescription}
              <button class="ghost small" onclick={startDescriptionEdit}>Edit</button>
            {/if}
          </div>
          {#if editingDescription}
            <textarea bind:value={descriptionDraft} rows="10" placeholder="Describe this project — goals, plans, notes, links…"></textarea>
            <div class="row-actions">
              <button class="ghost" onclick={() => (editingDescription = false)} disabled={savingDescription}>Cancel</button>
              <button class="primary" onclick={saveDescription} disabled={savingDescription}>
                {savingDescription ? "Saving…" : "Save"}
              </button>
            </div>
          {:else if project.description.trim() !== ""}
            <p class="description">{project.description}</p>
          {:else}
            <p class="muted">No description yet. Add goals, plans and notes so your team is on the same page.</p>
          {/if}
        </div>
      </div>

      <aside class="side">
        <div class="meta-item">
          <span class="label muted">Panel</span>
          <span>{panelName ?? "— not linked —"}</span>
        </div>
        <div class="meta-item">
          <span class="label muted">Server</span>
          <span class="mono">{project.server_identifier ?? "— not linked —"}</span>
        </div>
        <div class="meta-item">
          <span class="label muted">Deploy target</span>
          <span class="mono">{project.target_dir.trim() === "" ? "server root" : project.target_dir}</span>
        </div>
        <div class="meta-item">
          <span class="label muted">After deploy</span>
          <span>{project.post_deploy === "restart" ? "Restart server" : "Notify only"}</span>
        </div>
        <div class="meta-item">
          <span class="label muted">Created</span>
          <span>{formatDate(project.created_at)}{#if creatorName} · by {creatorName}{/if}</span>
        </div>
      </aside>
    </div>
  {:else}
    <form class="settings" onsubmit={saveSettings}>
      <div class="field">
        <label for="s-name">Name</label>
        <input id="s-name" bind:value={name} autocomplete="off" />
      </div>
      <div class="field">
        <label for="s-desc">Description</label>
        <textarea id="s-desc" bind:value={description} rows="4"></textarea>
      </div>

      <div class="two">
        <div class="field">
          <label for="s-panel">Panel</label>
          <select id="s-panel" bind:value={panelId}>
            <option value="">— none —</option>
            {#each panels as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <div class="field">
          <label for="s-server">Server identifier</label>
          <input id="s-server" bind:value={serverIdentifier} placeholder="e.g. d3aac109" spellcheck="false" autocomplete="off" />
        </div>
      </div>

      <div class="two">
        <div class="field">
          <label for="s-target">Deploy target directory <span class="muted">(empty = server root)</span></label>
          <input id="s-target" bind:value={targetDir} placeholder="e.g. plugins/MyPlugin" spellcheck="false" autocomplete="off" />
        </div>
        <div class="field">
          <label for="s-build">Build command <span class="muted">(optional)</span></label>
          <input id="s-build" bind:value={buildCommand} placeholder="e.g. npm run build" spellcheck="false" autocomplete="off" />
        </div>
      </div>

      <fieldset class="field">
        <legend>After a successful deploy</legend>
        <label class="radio"><input type="radio" bind:group={postDeploy} value="restart" /> Restart the server</label>
        <label class="radio"><input type="radio" bind:group={postDeploy} value="notify" /> Only show a notification</label>
      </fieldset>

      <label class="check">
        <input type="checkbox" bind:checked={autoBackup} />
        Back up the server before each deploy
      </label>

      <div class="row-actions end">
        <button type="button" class="ghost" onclick={() => (tab = "overview")} disabled={savingSettings}>Cancel</button>
        <button type="submit" class="primary" disabled={savingSettings || name.trim() === ""}>
          {savingSettings ? "Saving…" : "Save changes"}
        </button>
      </div>

      <div class="danger-zone">
        {#if confirmingDelete}
          <span class="muted">Delete this project for the whole team?</span>
          <button type="button" class="ghost" onclick={() => (confirmingDelete = false)} disabled={deleting}>Cancel</button>
          <button type="button" class="danger-btn" onclick={remove} disabled={deleting}>{deleting ? "Deleting…" : "Delete"}</button>
        {:else}
          <button type="button" class="ghost danger" onclick={() => (confirmingDelete = true)}>Delete project</button>
        {/if}
      </div>
    </form>
  {/if}
</div>

<style>
  .detail {
    max-width: 860px;
    margin: 22px auto 0;
  }

  .back {
    margin-bottom: 14px;
  }

  .project-head {
    margin-bottom: 14px;
  }

  h1 {
    font-size: 24px;
    margin-bottom: 8px;
  }

  .subline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .subtabs {
    display: flex;
    align-items: center;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 22px;
  }

  .subtabs button {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 8px 12px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
  }

  .subtabs button:hover {
    color: var(--text);
  }

  .subtabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .subtabs .soon {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-muted);
    opacity: 0.7;
  }

  .overview {
    display: grid;
    grid-template-columns: 1fr 240px;
    gap: 22px;
    align-items: start;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px;
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  h2 {
    font-size: 14px;
  }

  .description {
    line-height: 1.65;
    white-space: pre-wrap;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
  }

  .meta-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 13px;
  }

  .label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
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

  .row-actions {
    display: flex;
    gap: 10px;
    margin-top: 12px;
  }

  .row-actions.end {
    justify-content: flex-end;
    margin-top: 20px;
  }

  .small {
    padding: 3px 10px;
    font-size: 12px;
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
  }

  select {
    width: 100%;
  }

  .field {
    margin-bottom: 14px;
  }

  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
  }

  legend {
    color: var(--text-muted);
    font-size: 12px;
    padding: 0 4px;
  }

  .radio,
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .radio {
    margin-top: 6px;
  }

  .radio input,
  .check input {
    width: auto;
  }

  .check {
    margin: 4px 0 4px;
  }

  .danger-zone {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 26px;
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

  @media (max-width: 640px) {
    .overview,
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
