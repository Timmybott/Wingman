<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { deleteProject, saveProject } from "../api";
  import type { PostDeployAction, ProjectConfig, Server } from "../types";

  let {
    server,
    project,
    panelId,
    onSaved,
    onUnlinked,
    onClose,
  }: {
    server: Server;
    /** null = link a new project, otherwise edit the existing one. */
    project: ProjectConfig | null;
    panelId: string;
    onSaved: (project: ProjectConfig) => void;
    onUnlinked: (projectId: string) => void;
    onClose: () => void;
  } = $props();

  // The dialog is mounted fresh per open ({#if} in Dashboard), so seeding
  // the form once from the current project is exactly what we want.
  // svelte-ignore state_referenced_locally
  let localPath = $state(project?.local_path ?? "");
  // svelte-ignore state_referenced_locally
  let name = $state(project?.name ?? "");
  // svelte-ignore state_referenced_locally
  let targetDir = $state(project?.target_dir ?? "");
  // svelte-ignore state_referenced_locally
  let postDeploy = $state<PostDeployAction>(project?.post_deploy ?? "restart");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const canSave = $derived(localPath.trim() !== "" && !busy);

  function basename(path: string): string {
    return path.split(/[/\\]/).filter(Boolean).pop() ?? "";
  }

  async function chooseFolder() {
    const picked = await open({ directory: true, title: "Choose the project folder" });
    if (typeof picked === "string") {
      localPath = picked;
      if (name.trim() === "") name = basename(picked);
    }
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    try {
      const saved = await saveProject({
        id: project?.id ?? "",
        name,
        local_path: localPath,
        panel_id: panelId,
        server_identifier: server.identifier,
        target_dir: targetDir,
        build_command: project?.build_command ?? null,
        post_deploy: postDeploy,
        auto_backup: project?.auto_backup ?? true,
      });
      onSaved(saved);
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }

  async function unlink() {
    if (!project) return;
    busy = true;
    error = null;
    try {
      await deleteProject(project.id);
      onUnlinked(project.id);
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<button class="backdrop" aria-label="Close dialog" onclick={onClose}></button>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Project settings">
  <h2>{project ? "Project settings" : "Link a project"} — {server.name}</h2>
  <p class="muted">
    The linked folder is deployed to this server as a whole. Add a
    <code>.deployignore</code> (gitignore syntax) to exclude paths.
  </p>

  <form onsubmit={save}>
    <div class="field">
      <label for="folder">Project folder</label>
      <div class="folder-row">
        <input
          id="folder"
          bind:value={localPath}
          placeholder="/home/me/my-bot"
          spellcheck="false"
          autocomplete="off"
        />
        <button type="button" onclick={chooseFolder} disabled={busy}>Browse…</button>
      </div>
    </div>
    <div class="field">
      <label for="pname">Name <span class="muted">(optional, defaults to folder name)</span></label>
      <input id="pname" bind:value={name} autocomplete="off" />
    </div>
    <div class="field">
      <label for="target">Target directory on the server <span class="muted">(empty = server root)</span></label>
      <input id="target" bind:value={targetDir} placeholder="e.g. plugins/MyPlugin" spellcheck="false" autocomplete="off" />
    </div>
    <fieldset class="field">
      <legend>After a successful deploy</legend>
      <label class="radio">
        <input type="radio" bind:group={postDeploy} value="restart" />
        Restart the server
      </label>
      <label class="radio">
        <input type="radio" bind:group={postDeploy} value="notify" />
        Only show a notification
      </label>
    </fieldset>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="actions">
      {#if project}
        <button type="button" class="danger" onclick={unlink} disabled={busy}>
          Unlink project
        </button>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="ghost" onclick={onClose} disabled={busy}>Cancel</button>
      <button type="submit" class="primary" disabled={!canSave}>
        {project ? "Save" : "Link project"}
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
    border-radius: 0;
    cursor: default;
    z-index: 10;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(520px, 92vw);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 24px;
    z-index: 11;
  }

  h2 {
    font-size: 16px;
    margin-bottom: 6px;
  }

  p {
    margin: 0 0 16px;
    line-height: 1.5;
  }

  code {
    background: var(--surface-2);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 12px;
  }

  .field {
    margin-bottom: 14px;
  }

  .folder-row {
    display: flex;
    gap: 8px;
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

  .radio {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0;
    color: var(--text);
    font-size: 13px;
  }

  .radio input {
    width: auto;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 20px;
  }

  .spacer {
    flex: 1;
  }

  .danger {
    color: var(--danger);
  }
</style>
