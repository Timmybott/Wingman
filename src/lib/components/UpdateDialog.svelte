<script lang="ts">
  import type { Update } from "@tauri-apps/plugin-updater";

  let {
    update,
    onInstall,
    onLater,
  }: {
    update: Update;
    onInstall: () => Promise<void>;
    onLater: () => void;
  } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function install() {
    busy = true;
    error = null;
    try {
      await onInstall();
      // On success the app relaunches — this code path usually never returns.
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }
</script>

<button class="backdrop" aria-label="Dismiss update" onclick={() => !busy && onLater()}></button>
<div class="dialog" role="dialog" aria-modal="true" aria-label="Update available">
  <div class="rocket">🚀</div>
  <h2>Update available</h2>
  <p>
    <strong>Wingman {update.version}</strong> is ready to install.
    The app restarts automatically afterwards.
  </p>
  {#if update.body}
    <p class="notes muted">{update.body.split("\n").slice(0, 4).join("\n")}</p>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <div class="actions">
    <button class="ghost" onclick={onLater} disabled={busy}>Later</button>
    <button class="primary" onclick={install} disabled={busy}>
      {busy ? "Installing…" : "Install & restart"}
    </button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    border: none;
    border-radius: 0;
    cursor: default;
    z-index: 20;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(400px, 90vw);
    background: var(--surface);
    border: 1px solid var(--accent);
    border-radius: 12px;
    padding: 28px;
    text-align: center;
    z-index: 21;
  }

  .rocket {
    font-size: 34px;
    margin-bottom: 8px;
  }

  h2 {
    font-size: 17px;
    margin-bottom: 8px;
  }

  p {
    margin: 0 0 12px;
    line-height: 1.5;
  }

  .notes {
    font-size: 12.5px;
    white-space: pre-wrap;
    text-align: left;
    background: var(--surface-2);
    border-radius: 8px;
    padding: 10px 12px;
  }

  .actions {
    display: flex;
    justify-content: center;
    gap: 10px;
    margin-top: 16px;
  }
</style>
