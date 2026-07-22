<script lang="ts">
  import {
    deleteProject,
    updateProject,
    type CloudPanel,
    type CloudProject,
    type PostDeploy,
    type TeamMember,
  } from "../cloud";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getProjectPath, removeProjectPath, setProjectPath } from "../api";
  import { toggleTaskInMarkdown } from "../markdown";
  import DeployPanel from "./DeployPanel.svelte";
  import FileBrowser from "./FileBrowser.svelte";
  import IssuesPanel from "./IssuesPanel.svelte";
  import Markdown from "./Markdown.svelte";

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

  type Tab = "overview" | "issues" | "deploy" | "files" | "settings";
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

  // Per-device local folder binding.
  let localPath = $state<string | null>(null);
  let pathBusy = $state(false);

  $effect(() => {
    const id = project.id;
    getProjectPath(id)
      .then((p) => (localPath = p))
      .catch(() => (localPath = null));
  });

  async function chooseFolder() {
    const picked = await open({ directory: true, title: "Choose the local project folder" });
    if (typeof picked !== "string") return;
    pathBusy = true;
    error = null;
    try {
      await setProjectPath(project.id, picked);
      localPath = picked;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      pathBusy = false;
    }
  }

  async function unlinkFolder() {
    pathBusy = true;
    error = null;
    try {
      await removeProjectPath(project.id);
      localPath = null;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      pathBusy = false;
    }
  }

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

  /** Tick a checklist item straight from the rendered description and persist. */
  async function toggleTask(index: number, checked: boolean) {
    const next = toggleTaskInMarkdown(project.description, index, checked);
    error = null;
    try {
      const updated = await updateProject(project.id, { description: next });
      onChanged(updated);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
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
    <button class:active={tab === "issues"} onclick={() => (tab = "issues")}>Issues</button>
    <button class:active={tab === "deploy"} onclick={() => (tab = "deploy")}>Deploy</button>
    <button class:active={tab === "files"} onclick={() => (tab = "files")}>Files</button>
    <button class:active={tab === "settings"} onclick={openSettings}>Settings</button>
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
            <textarea bind:value={descriptionDraft} rows="10" placeholder="Describe this project — goals, plans, notes, links…&#10;&#10;Markdown supported: # headings, **bold**, - lists, `code`, and&#10;- [ ] checklists you can tick right on the overview"></textarea>
            <p class="hint muted">Markdown supported — headings, lists, code, links, and <code>- [ ]</code> checklists.</p>
            <div class="row-actions">
              <button class="ghost" onclick={() => (editingDescription = false)} disabled={savingDescription}>Cancel</button>
              <button class="primary" onclick={saveDescription} disabled={savingDescription}>
                {savingDescription ? "Saving…" : "Save"}
              </button>
            </div>
          {:else if project.description.trim() !== ""}
            <Markdown source={project.description} onToggleTask={toggleTask} />
          {:else}
            <p class="muted">No description yet. Add goals, plans and notes so your team is on the same page.</p>
          {/if}
        </div>

        <div class="card">
          <div class="card-head">
            <h2>Local folder · this device</h2>
          </div>
          {#if localPath}
            <p class="folder mono">{localPath}</p>
            <div class="row-actions">
              <button class="ghost small" onclick={chooseFolder} disabled={pathBusy}>Change…</button>
              <button class="ghost small" onclick={unlinkFolder} disabled={pathBusy}>Unlink</button>
            </div>
          {:else}
            <p class="muted">
              Not set on this device. Link a folder to deploy this project from
              here — each teammate picks their own (or none).
            </p>
            <div class="row-actions">
              <button class="primary small" onclick={chooseFolder} disabled={pathBusy}>Choose folder…</button>
            </div>
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
  {:else if tab === "issues"}
    <IssuesPanel projectId={project.id} />
  {:else if tab === "deploy"}
    <DeployPanel {project} {localPath} />
  {:else if tab === "files"}
    {#if project.panel_id && project.server_identifier}
      <FileBrowser panelId={project.panel_id} identifier={project.server_identifier} />
    {:else}
      <p class="muted center empty">This project isn't linked to a server, so there are no files to browse.</p>
    {/if}
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

  .overview {
    display: grid;
    grid-template-columns: 1fr 240px;
    gap: 22px;
    align-items: start;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
  }

  .folder {
    word-break: break-all;
    margin-bottom: 8px;
    font-size: 13px;
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

  .hint {
    font-size: 12px;
    margin: 8px 0 0;
  }

  .hint code {
    background: var(--surface-2);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 11px;
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

  .empty {
    padding: 32px 0;
    max-width: 420px;
    margin: 0 auto;
    line-height: 1.5;
  }

  @media (max-width: 640px) {
    .overview,
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
