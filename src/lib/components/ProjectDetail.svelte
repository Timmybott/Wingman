<script lang="ts">
  import {
    deleteProject,
    listDeploys,
    listIssues,
    requestProjectDeletion,
    updateProject,
    type CloudPanel,
    type CloudProject,
    type DeployEntry,
    type Issue,
    type PostDeploy,
    type TeamMember,
  } from "../cloud";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    getProjectPath,
    removeLocalProject,
    removeProjectPath,
    setProjectPath,
  } from "../api";
  import { toggleTaskInMarkdown } from "../markdown";
  import DeployPanel from "./DeployPanel.svelte";
  import FileBrowser from "./FileBrowser.svelte";
  import IssuesPanel from "./IssuesPanel.svelte";
  import Markdown from "./Markdown.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  let {
    project,
    panels,
    members,
    teamName,
    onBack,
    onChanged,
    onDeleted,
    onOpenServer,
    onOpenTeam,
    onOpenProfile,
  }: {
    project: CloudProject;
    panels: CloudPanel[];
    members: TeamMember[];
    teamName: string;
    onBack: () => void;
    onChanged: (updated: CloudProject) => void;
    onDeleted: (id: string) => void;
    onOpenServer: (panelId: string, identifier: string) => void;
    onOpenTeam: () => void;
    onOpenProfile: (userId: string) => void;
  } = $props();

  // Only offer the jump-to-Panels shortcut when the project actually imports a
  // server (both the panel and the server identifier are set).
  const linkedServer = $derived(
    project.panel_id && project.server_identifier
      ? { panelId: project.panel_id, identifier: project.server_identifier }
      : null,
  );

  function openServer() {
    if (linkedServer) onOpenServer(linkedServer.panelId, linkedServer.identifier);
  }

  type Tab = "overview" | "issues" | "deploy" | "files" | "settings";
  let tab = $state<Tab>("overview");

  let error = $state<string | null>(null);

  // Overview summary data (issues + deploys) for the GitHub-style header.
  let issues = $state<Issue[]>([]);
  let deploys = $state<DeployEntry[]>([]);

  async function loadStats() {
    const id = project.id;
    try {
      [issues, deploys] = await Promise.all([listIssues(id), listDeploys(id)]);
    } catch (e) {
      console.error("could not load project stats:", e);
    }
  }

  // Reload the Overview's stats whenever it's shown (and on project change), so
  // a deploy made in another tab is reflected the moment you come back — the
  // stats were previously loaded only once on mount and went stale.
  $effect(() => {
    void project.id;
    if (tab === "overview") void loadStats();
  });

  const openIssueCount = $derived(issues.filter((i) => i.status === "open").length);
  const deployCount = $derived(deploys.length);
  // Deploys come back newest-first.
  const lastDeploy = $derived<DeployEntry | null>(deploys[0] ?? null);
  const lastSuccess = $derived<DeployEntry | null>(
    deploys.find((d) => d.status === "success") ?? null,
  );
  const currentCommit = $derived(lastSuccess?.commit ?? null);
  const recentDeploys = $derived(deploys.slice(0, 4));

  // Overview: quick inline description edit.
  let editingDescription = $state(false);
  let descriptionDraft = $state("");
  let savingDescription = $state(false);

  // Settings form buffer.
  let name = $state("");
  let description = $state("");
  let logoUrl = $state("");
  let panelId = $state<string>("");
  let serverIdentifier = $state("");
  let targetDir = $state("");
  let buildCommand = $state("");
  let postDeploy = $state<PostDeploy>("restart");
  let autoBackup = $state(true);
  let savingSettings = $state(false);

  let dangerMode = $state<null | "feather" | "everywhere">(null);
  let deleting = $state(false);

  // Per-device local folder binding.
  let localPath = $state<string | null>(null);
  let pathBusy = $state(false);
  // When a folder is linked to an empty directory, the Deploy tab imports the
  // server's files into it once — set here, consumed by DeployPanel.
  let autoImport = $state(false);

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
      const empty = await setProjectPath(project.id, picked);
      localPath = picked;
      // Fill an empty folder straight from the server so the diff is
      // meaningful immediately — the import runs (with progress) in Deploy.
      if (empty && project.panel_id && project.server_identifier) {
        autoImport = true;
        tab = "deploy";
      }
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
    logoUrl = project.logo_url ?? "";
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
        logo_url: logoUrl.trim() || null,
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

  /** Delete the cloud project; keep local files on every device. */
  async function removeFromFeather() {
    deleting = true;
    error = null;
    try {
      await deleteProject(project.id);
      await removeLocalProject(project.id, false);
      onDeleted(project.id);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
      deleting = false;
    }
  }

  /** Tombstone + delete the project, and delete local folders everywhere. */
  async function deleteEverywhere() {
    deleting = true;
    error = null;
    try {
      await requestProjectDeletion(project.id);
      await removeLocalProject(project.id, true);
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

  /** Compact "3h ago" / "2d ago" for activity lines. */
  function relativeTime(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const sec = Math.round(diff / 1000);
    if (sec < 60) return "just now";
    const min = Math.round(sec / 60);
    if (min < 60) return `${min}m ago`;
    const hr = Math.round(min / 60);
    if (hr < 24) return `${hr}h ago`;
    const day = Math.round(hr / 24);
    if (day < 30) return `${day}d ago`;
    return formatDate(iso);
  }

  function deployActor(d: DeployEntry): string {
    return d.display_name?.trim() || d.username || "someone";
  }

</script>

<div class="detail">
  <button class="back ghost" onclick={onBack}>← All projects</button>

  <header class="project-head">
    <div class="head-main">
      {#if project.logo_url}
        <img class="proj-logo" src={project.logo_url} alt={project.name} />
      {:else}
        <span class="proj-logo placeholder">{project.name.charAt(0).toUpperCase()}</span>
      {/if}
      <div class="head-text">
        <h1>{project.name}</h1>
        <div class="subline">
          <button class="team-chip" onclick={() => onOpenTeam()} title="Open the team page">{teamName}</button>
          {#if panelName}
            <span class="tag">{panelName}</span>
          {/if}
          {#if project.server_identifier}
            <span class="tag mono">{project.server_identifier}</span>
          {:else}
            <span class="muted">Not linked to a server yet</span>
          {/if}
          {#if linkedServer}
            <button class="ghost small open-panels" onclick={openServer} title="Show this server's tile in the Panels tab">
              Open in Panels ↗
            </button>
          {/if}
        </div>
      </div>
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
    <div class="stats">
      <button class="stat" onclick={() => (tab = "issues")} title="Open issues">
        <span class="stat-num">{openIssueCount}</span>
        <span class="stat-label muted">Open {openIssueCount === 1 ? "issue" : "issues"}</span>
      </button>
      <button class="stat" onclick={() => (tab = "deploy")} title="Deploy history">
        <span class="stat-num">{deployCount}</span>
        <span class="stat-label muted">{deployCount === 1 ? "Deploy" : "Deploys"}</span>
      </button>
      <button class="stat" onclick={() => (tab = "deploy")} title="Most recent deploy">
        {#if lastDeploy}
          <span class="stat-num sm">
            <span class="dot {lastDeploy.status}"></span>
            {relativeTime(lastDeploy.created_at)}
          </span>
          <span class="stat-label muted">Last {lastDeploy.kind}</span>
        {:else}
          <span class="stat-num sm muted">—</span>
          <span class="stat-label muted">No deploys yet</span>
        {/if}
      </button>
      <button class="stat" onclick={() => (tab = "deploy")} title="Commit currently on the server">
        {#if currentCommit}
          <span class="stat-num sm mono">{currentCommit}</span>
          <span class="stat-label muted">Current commit</span>
        {:else}
          <span class="stat-num sm muted">—</span>
          <span class="stat-label muted">Not deployed</span>
        {/if}
      </button>
    </div>

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
            <MarkdownEditor bind:value={descriptionDraft} rows={10} placeholder="Describe this project — goals, plans, notes, links…" />
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
            <h2>Recent activity</h2>
            {#if recentDeploys.length > 0}
              <button class="ghost small" onclick={() => (tab = "deploy")}>View all</button>
            {/if}
          </div>
          {#if recentDeploys.length > 0}
            <ul class="activity">
              {#each recentDeploys as d (d.id)}
                <li>
                  <span class="a-badge {d.status}">{d.status === "success" ? "✓" : "✕"}</span>
                  <div class="a-main">
                    <span class="a-title">
                      <span class="a-kind">{d.kind}</span>
                      {#if d.commit_summary}<span class="a-summary">{d.commit_summary}</span>
                      {:else if d.status === "failed" && d.message}<span class="a-summary fail">{d.message}</span>{/if}
                    </span>
                    <span class="a-meta muted">
                      {#if d.commit}<span class="mono">{d.commit}</span> · {/if}
                      {deployActor(d)} · {relativeTime(d.created_at)}
                    </span>
                  </div>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">No deploys yet. The team's deploys and rollbacks show up here.</p>
          {/if}
        </div>
      </div>

      <aside class="side">
        <div class="meta-item">
          <span class="label muted">Team</span>
          <button class="link-btn" onclick={() => onOpenTeam()} title="Open the team page">{teamName} ↗</button>
        </div>
        <div class="meta-item">
          <span class="label muted">Local folder · this device</span>
          {#if localPath}
            <span class="mono folder">{localPath}</span>
          {:else}
            <span class="muted">Not set — add it in <button class="inline-link" onclick={openSettings}>Settings</button></span>
          {/if}
        </div>
        <div class="meta-item">
          <span class="label muted">Panel</span>
          <span>{panelName ?? "— not linked —"}</span>
        </div>
        <div class="meta-item">
          <span class="label muted">Server</span>
          {#if linkedServer}
            <button class="link-btn mono" onclick={openServer} title="Show this server's tile in the Panels tab">
              {project.server_identifier} ↗
            </button>
          {:else}
            <span class="mono">{project.server_identifier ?? "— not linked —"}</span>
          {/if}
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
          <span>
            {formatDate(project.created_at)}{#if creatorName && creator} · by
              <button class="inline-link" onclick={() => onOpenProfile(creator.user_id)}>{creatorName}</button>
            {:else if creatorName} · by {creatorName}{/if}
          </span>
        </div>
      </aside>
    </div>
  {:else if tab === "issues"}
    <IssuesPanel projectId={project.id} {onOpenProfile} />
  {:else if tab === "deploy"}
    <DeployPanel {project} {localPath} {autoImport} onImported={() => (autoImport = false)} />
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
        <MarkdownEditor id="s-desc" bind:value={description} rows={4} />
      </div>
      <div class="field">
        <label for="s-logo">Logo image URL <span class="muted">(optional)</span></label>
        <input id="s-logo" bind:value={logoUrl} placeholder="https://…/logo.png" spellcheck="false" autocomplete="off" />
      </div>

      <div class="two">
        <div class="field">
          <label for="s-panel">Panel</label>
          <select id="s-panel" bind:value={panelId}>
            <option value="" disabled>Select a panel…</option>
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

      <fieldset class="local-folder">
        <legend>Local folder · this device</legend>
        {#if localPath}
          <p class="folder mono">{localPath}</p>
          <div class="row-actions">
            <button type="button" class="ghost small" onclick={chooseFolder} disabled={pathBusy}>Change…</button>
            <button type="button" class="ghost small" onclick={unlinkFolder} disabled={pathBusy}>Unlink</button>
          </div>
        {:else}
          <p class="muted">
            Not set on this device. Link a folder to deploy this project from here —
            each teammate picks their own (or none). Linking an empty folder imports
            the server's current files into it automatically.
          </p>
          <div class="row-actions">
            <button type="button" class="primary small" onclick={chooseFolder} disabled={pathBusy}>Choose folder…</button>
          </div>
        {/if}
      </fieldset>

      <div class="danger-zone">
        <h3>Danger zone</h3>
        {#if dangerMode === null}
          <div class="danger-row">
            <div class="danger-text">
              <strong>Remove from Feather</strong>
              <p class="muted">Deletes the project, its issues and deploy history for the team. Local files on every device are kept.</p>
            </div>
            <button type="button" class="ghost danger" onclick={() => (dangerMode = "feather")}>Remove</button>
          </div>
          <div class="danger-row">
            <div class="danger-text">
              <strong>Delete everywhere</strong>
              <p class="muted">Also deletes the linked local folder on every teammate's machine on their next launch. Permanent.</p>
            </div>
            <button type="button" class="ghost danger" onclick={() => (dangerMode = "everywhere")}>Delete everywhere</button>
          </div>
        {:else if dangerMode === "feather"}
          <p>Remove “{project.name}” from Feather? Local files stay on every device.</p>
          <div class="row-actions">
            <button type="button" class="ghost" onclick={() => (dangerMode = null)} disabled={deleting}>Cancel</button>
            <button type="button" class="danger-btn" onclick={removeFromFeather} disabled={deleting}>
              {deleting ? "Removing…" : "Remove from Feather"}
            </button>
          </div>
        {:else}
          <p>
            Delete “{project.name}” <strong>everywhere</strong>, including the linked local folder on
            every teammate's device? This cannot be undone.
          </p>
          <div class="row-actions">
            <button type="button" class="ghost" onclick={() => (dangerMode = null)} disabled={deleting}>Cancel</button>
            <button type="button" class="danger-btn" onclick={deleteEverywhere} disabled={deleting}>
              {deleting ? "Deleting…" : "Delete everywhere"}
            </button>
          </div>
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

  .head-main {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .proj-logo {
    flex-shrink: 0;
    width: 52px;
    height: 52px;
    border-radius: 11px;
    object-fit: cover;
    border: 1px solid var(--border);
  }

  .proj-logo.placeholder {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    font-weight: 700;
    font-size: 22px;
  }

  .head-text {
    min-width: 0;
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
    flex-wrap: wrap;
  }

  .team-chip {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 3px 11px;
    font-size: 12px;
    font-weight: 600;
  }

  .team-chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .inline-link {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--accent);
    font: inherit;
  }

  .inline-link:hover {
    text-decoration: underline;
  }

  .open-panels {
    padding: 2px 10px;
    font-size: 12px;
  }

  .link-btn {
    align-self: flex-start;
    background: transparent;
    border: none;
    border-radius: 0;
    padding: 0;
    color: var(--accent);
    font-size: 13px;
    text-align: left;
  }

  .link-btn:hover {
    text-decoration: underline;
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

  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    margin-bottom: 22px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
  }

  .stat:hover {
    border-color: var(--accent);
  }

  .stat-num {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.1;
  }

  .stat-num.sm {
    font-size: 15px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .stat-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot.success {
    background: var(--ok, #34d399);
  }

  .dot.failed {
    background: var(--danger, #f87171);
  }

  .activity {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .activity li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .a-badge {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 700;
  }

  .a-badge.success {
    background: #10b98122;
    color: #34d399;
  }

  .a-badge.failed {
    background: #ef444422;
    color: #f87171;
  }

  .a-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .a-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .a-kind {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .a-summary {
    font-weight: 600;
    font-size: 13px;
  }

  .a-summary.fail {
    font-weight: 400;
    color: var(--text-muted);
  }

  .a-meta {
    font-size: 12px;
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

  .local-folder {
    margin-top: 22px;
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
    margin-top: 26px;
    padding-top: 18px;
    border-top: 1px solid var(--border);
  }

  .danger-zone h3 {
    color: var(--danger, #f87171);
    margin-bottom: 12px;
  }

  .danger-zone > p {
    line-height: 1.5;
    margin-bottom: 12px;
  }

  .danger-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }

  .danger-row + .danger-row {
    border-top: 1px solid var(--border);
  }

  .danger-text p {
    margin: 3px 0 0;
    font-size: 12px;
    line-height: 1.45;
    max-width: 46ch;
  }

  .danger-row .danger {
    flex-shrink: 0;
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

    .stats {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
