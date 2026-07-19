<script lang="ts">
  import { savePanel, testConnection } from "../api";
  import type { PanelConfig } from "../types";

  let { onConnected }: { onConnected: (panel: PanelConfig) => void } = $props();

  let name = $state("");
  let baseUrl = $state("");
  let apiKey = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let testResult = $state<number | null>(null);

  const canSubmit = $derived(baseUrl.trim() !== "" && apiKey.trim() !== "" && !busy);

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

  async function connect(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    try {
      onConnected(await savePanel(name, baseUrl, apiKey));
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }
</script>

<div class="setup">
  <h2>Connect your panel</h2>
  <p class="muted">
    Wingman talks to Pterodactyl through the client API. Create an API key in your
    panel under <strong>Account → API Credentials</strong> — the key is stored in
    your operating system's keychain, never in plain text.
  </p>

  <form onsubmit={connect}>
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
      <input
        id="key"
        type="password"
        bind:value={apiKey}
        placeholder="ptlc_…"
        autocomplete="off"
      />
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
      <button type="button" onclick={test} disabled={!canSubmit}>Test connection</button>
      <button type="submit" class="primary" disabled={!canSubmit}>
        {busy ? "Connecting…" : "Connect"}
      </button>
    </div>
  </form>
</div>

<style>
  .setup {
    max-width: 460px;
    margin: 48px auto 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 28px;
  }

  h2 {
    font-size: 18px;
    margin-bottom: 8px;
  }

  p {
    margin: 0 0 18px;
    line-height: 1.5;
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
