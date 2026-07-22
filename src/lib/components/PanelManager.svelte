<script lang="ts">
  import { testConnection } from "../api";
  import { createPanel, deletePanel, type CloudPanel } from "../cloud";

  let {
    panels,
    teamId,
    activating,
    activateError,
    onActivate,
    onChanged,
  }: {
    panels: CloudPanel[];
    teamId: string;
    activating: boolean;
    activateError: string | null;
    onActivate: (panel: CloudPanel) => void;
    onChanged: () => Promise<void>;
  } = $props();

  let showForm = $state(false);
  let name = $state("");
  let baseUrl = $state("");
  let apiKey = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let testResult = $state<number | null>(null);
  let deletingId = $state<string | null>(null);

  const canSubmit = $derived(baseUrl.trim() !== "" && apiKey.trim() !== "" && !busy);

  /** Explicit name, else the URL host, else the raw URL. */
  function label(): string {
    if (name.trim() !== "") return name.trim();
    const url = baseUrl.trim();
    try {
      return new URL(url.includes("://") ? url : `https://${url}`).host;
    } catch {
      return url;
    }
  }

  function resetForm() {
    name = "";
    baseUrl = "";
    apiKey = "";
    error = null;
    testResult = null;
  }

  async function test() {
    busy = true;
    error = null;
    testResult = null;
    try {
      testResult = await testConnection(baseUrl, apiKey);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function add(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    try {
      await createPanel(teamId, label(), baseUrl.trim(), apiKey.trim());
      resetForm();
      showForm = false;
      await onChanged();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      busy = false;
    }
  }

  async function remove(panel: CloudPanel) {
    if (deletingId) return;
    deletingId = panel.id;
    error = null;
    try {
      await deletePanel(panel.id);
      await onChanged();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      deletingId = null;
    }
  }
</script>

<div class="panels">
  <div class="head">
    <div>
      <h2>Panels</h2>
      <p class="muted">
        Pterodactyl connections shared with your team. Keys are encrypted in the
        cloud — everyone on the team can deploy, no one has to re-enter them.
      </p>
    </div>
    {#if !showForm}
      <button class="primary" onclick={() => (showForm = true)}>Add panel</button>
    {/if}
  </div>

  {#if activateError}
    <p class="error">{activateError}</p>
  {/if}

  {#if panels.length > 0}
    <ul class="list">
      {#each panels as panel (panel.id)}
        <li>
          <div class="info">
            <span class="name">{panel.name}</span>
            <span class="muted url">{panel.base_url}</span>
          </div>
          <div class="row-actions">
            <button
              class="primary"
              disabled={activating || deletingId !== null}
              onclick={() => onActivate(panel)}
            >
              {activating ? "Connecting…" : "Connect"}
            </button>
            <button
              class="ghost danger"
              title="Remove panel"
              disabled={deletingId !== null}
              onclick={() => remove(panel)}
            >
              {deletingId === panel.id ? "…" : "Remove"}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {:else if !showForm}
    <p class="empty muted">No panels yet. Add your first Pterodactyl connection.</p>
  {/if}

  {#if showForm}
    <form onsubmit={add}>
      <h3>Add a panel</h3>
      <p class="muted">
        Create an API key in your panel under <strong>Account → API Credentials</strong>.
      </p>
      <div class="field">
        <label for="url">Panel URL</label>
        <input
          id="url"
          bind:value={baseUrl}
          placeholder="https://panel.example.com"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="field">
        <label for="key">API key</label>
        <input id="key" type="password" bind:value={apiKey} placeholder="ptlc_…" autocomplete="off" />
      </div>
      <div class="field">
        <label for="name">Display name <span class="muted">(optional)</span></label>
        <input id="name" bind:value={name} placeholder="My panel" autocomplete="off" />
      </div>

      {#if error}
        <p class="error">{error}</p>
      {:else if testResult !== null}
        <p class="ok">
          Connection works — {testResult}
          {testResult === 1 ? "server" : "servers"} visible.
        </p>
      {/if}

      <div class="actions">
        <button
          type="button"
          class="ghost"
          onclick={() => {
            showForm = false;
            resetForm();
          }}
        >
          Cancel
        </button>
        <button type="button" onclick={test} disabled={!canSubmit}>Test</button>
        <button type="submit" class="primary" disabled={!canSubmit}>
          {busy ? "Saving…" : "Add panel"}
        </button>
      </div>
    </form>
  {/if}
</div>

<style>
  .panels {
    max-width: 620px;
    margin: 40px auto 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  h2 {
    font-size: 18px;
    margin-bottom: 6px;
  }

  p {
    margin: 0;
    line-height: 1.5;
  }

  .head .primary {
    flex-shrink: 0;
  }

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }

  .name {
    font-weight: 600;
  }

  .url {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .danger:hover {
    color: var(--danger, #f87171);
    border-color: var(--danger, #f87171);
  }

  .empty {
    text-align: center;
    padding: 28px 0;
  }

  form {
    margin-top: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 22px;
  }

  h3 {
    font-size: 15px;
    margin-bottom: 6px;
  }

  form p {
    margin-bottom: 16px;
  }

  .field {
    margin-bottom: 14px;
  }

  .actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 20px;
  }
</style>
