<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { listServers, setProjectPath } from "../api";
  import { createProject, type CloudPanel, type CloudProject } from "../cloud";
  import { formatMib } from "../format";
  import type { Server } from "../types";

  let {
    teamId,
    panels,
    existing,
    onCreated,
    onClose,
  }: {
    teamId: string;
    panels: CloudPanel[];
    existing: CloudProject[];
    onCreated: (project: CloudProject) => void;
    onClose: () => void;
  } = $props();

  // Mounted fresh each time the dialog opens, so defaulting to the only panel
  // (when there's just one) from the initial prop is intended.
  // svelte-ignore state_referenced_locally
  let panelId = $state(panels.length === 1 ? panels[0].id : "");
  let servers = $state<Server[]>([]);
  let loadingServers = $state(false);
  let serverError = $state<string | null>(null);
  let selected = $state<Server | null>(null);

  let name = $state("");
  let description = $state("");
  let localPath = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const canCreate = $derived(
    panelId !== "" && selected !== null && name.trim() !== "" && !busy,
  );

  function isImported(server: Server): boolean {
    return existing.some(
      (p) => p.panel_id === panelId && p.server_identifier === server.identifier,
    );
  }

  async function loadServers() {
    selected = null;
    servers = [];
    if (panelId === "") return;
    loadingServers = true;
    serverError = null;
    try {
      servers = await listServers(panelId);
    } catch (e) {
      serverError = String(e);
    } finally {
      loadingServers = false;
    }
  }

  // Reload servers whenever the chosen panel changes.
  $effect(() => {
    void panelId;
    void loadServers();
  });

  function pick(server: Server) {
    if (isImported(server)) return;
    selected = server;
    if (name.trim() === "") name = server.name;
  }

  function limits(server: Server): string {
    const mem = server.limits.memory ? formatMib(server.limits.memory) : "∞ RAM";
    const disk = server.limits.disk ? formatMib(server.limits.disk) : "∞ disk";
    const cpu = server.limits.cpu ? `${server.limits.cpu}% CPU` : "∞ CPU";
    return `${mem} · ${disk} · ${cpu}`;
  }

  async function chooseFolder() {
    const picked = await open({ directory: true, title: "Choose the local project folder" });
    if (typeof picked === "string") localPath = picked;
  }

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (!selected) return;
    busy = true;
    error = null;
    try {
      const project = await createProject(teamId, {
        name: name.trim(),
        description: description.trim(),
        panel_id: panelId,
        server_identifier: selected.identifier,
      });
      if (localPath.trim() !== "") {
        try {
          await setProjectPath(project.id, localPath.trim());
        } catch (e) {
          // The project exists; surface the folder problem but continue.
          error = `Project created, but the folder could not be linked: ${e}`;
        }
      }
      onCreated(project);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />
<button class="backdrop" aria-label="Close" onclick={onClose}></button>

<div class="dialog" role="dialog" aria-modal="true" aria-label="New project">
  <h2>New project</h2>
  <p class="muted">
    A project imports one of your panel's servers so you can plan, deploy and
    track it. Pick a panel and a server; a local folder is optional (add it now
    or later to deploy from this device).
  </p>

  <form onsubmit={create}>
    <div class="field">
      <label for="panel">Panel</label>
      <select id="panel" bind:value={panelId}>
        <option value="" disabled>Select a panel…</option>
        {#each panels as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    <div class="field">
      <span class="label">Server</span>
      {#if panelId === ""}
        <p class="hint muted">Choose a panel first.</p>
      {:else if loadingServers}
        <p class="hint muted">Loading servers…</p>
      {:else if serverError}
        <p class="error">{serverError}</p>
      {:else if servers.length === 0}
        <p class="hint muted">This panel has no servers.</p>
      {:else}
        <ul class="servers">
          {#each servers as server (server.identifier)}
            {@const imported = isImported(server)}
            <li>
              <button
                type="button"
                class="server"
                class:selected={selected?.identifier === server.identifier}
                class:imported
                disabled={imported}
                onclick={() => pick(server)}
              >
                <span class="s-main">
                  <span class="s-name">{server.name}</span>
                  <span class="muted s-meta">{limits(server)}</span>
                </span>
                {#if imported}
                  <span class="muted tag">already a project</span>
                {:else if selected?.identifier === server.identifier}
                  <span class="tag on">selected</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="field">
      <label for="pname">Name</label>
      <input id="pname" bind:value={name} placeholder="Project name" autocomplete="off" />
    </div>

    <div class="field">
      <label for="pdesc">Description <span class="muted">(optional)</span></label>
      <textarea id="pdesc" bind:value={description} rows="2" placeholder="What's the plan?"></textarea>
    </div>

    <div class="field">
      <span class="label">Local folder <span class="muted">(optional — needed to deploy from this device)</span></span>
      <div class="folder-row">
        <input bind:value={localPath} placeholder="/home/me/my-server" spellcheck="false" autocomplete="off" />
        <button type="button" onclick={chooseFolder} disabled={busy}>Browse…</button>
        {#if localPath !== ""}
          <button type="button" class="ghost" onclick={() => (localPath = "")} disabled={busy}>Clear</button>
        {/if}
      </div>
    </div>

    {#if error}<p class="error">{error}</p>{/if}

    <div class="actions">
      <button type="button" class="ghost" onclick={onClose} disabled={busy}>Cancel</button>
      <button type="submit" class="primary" disabled={!canCreate}>
        {busy ? "Creating…" : "Create project"}
      </button>
    </div>
  </form>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    border: none;
    z-index: 10;
    cursor: default;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, 94vw);
    max-height: 90vh;
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 24px;
    z-index: 11;
  }

  h2 {
    font-size: 18px;
    margin-bottom: 6px;
  }

  p {
    margin: 0 0 16px;
    line-height: 1.5;
  }

  .field {
    margin-bottom: 14px;
  }

  .label {
    display: block;
    margin-bottom: 6px;
    font-size: 13px;
  }

  select,
  textarea {
    width: 100%;
    font: inherit;
  }

  textarea {
    resize: vertical;
  }

  .hint {
    margin: 0;
    font-size: 13px;
  }

  .servers {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
  }

  .server {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    text-align: left;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
  }

  .server:hover:not(:disabled) {
    border-color: var(--accent);
  }

  .server.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, var(--surface-2));
  }

  .server.imported {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .s-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .s-name {
    font-weight: 600;
    font-size: 14px;
  }

  .s-meta {
    font-size: 12px;
  }

  .tag {
    flex-shrink: 0;
    font-size: 11px;
  }

  .tag.on {
    color: var(--accent);
    font-weight: 600;
  }

  .folder-row {
    display: flex;
    gap: 8px;
  }

  .folder-row input {
    flex: 1;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 20px;
  }
</style>
