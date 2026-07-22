<script lang="ts">
  import { onMount } from "svelte";
  import { readServerFile, writeServerFile } from "../api";

  let {
    panelId,
    identifier,
    path,
    size,
    onClose,
    onSaved,
  }: {
    panelId: string;
    identifier: string;
    path: string;
    size: number;
    onClose: () => void;
    onSaved?: () => void;
  } = $props();

  // Files above this are not loaded into the editor (a textarea is the wrong
  // tool for megabytes of data).
  const MAX_EDIT_BYTES = 1_000_000;

  let content = $state("");
  let original = "";
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let readonlyReason = $state<string | null>(null);

  const dirty = $derived(readonlyReason === null && content !== original);

  onMount(async () => {
    if (size > MAX_EDIT_BYTES) {
      readonlyReason = "This file is too large to edit here.";
      loading = false;
      return;
    }
    try {
      content = await readServerFile(panelId, identifier, path);
      original = content;
    } catch (e) {
      readonlyReason = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  });

  async function save() {
    saving = true;
    error = null;
    try {
      await writeServerFile(panelId, identifier, path, content);
      original = content;
      onSaved?.();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      saving = false;
    }
  }

  function tryClose() {
    if (!dirty || confirm("Discard unsaved changes?")) onClose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && tryClose()} />

<button class="backdrop" aria-label="Close editor" onclick={tryClose}></button>
<div class="modal" role="dialog" aria-modal="true" aria-label="Edit file">
  <header>
    <span class="path mono">{path}{#if dirty} <span class="dot">●</span>{/if}</span>
    <button class="ghost" onclick={tryClose} title="Close (Esc)">✕</button>
  </header>

  {#if loading}
    <p class="muted pad">Loading…</p>
  {:else if readonlyReason}
    <p class="muted pad">{readonlyReason}</p>
  {:else}
    <textarea bind:value={content} spellcheck="false" autocomplete="off"></textarea>
  {/if}

  {#if error}<p class="error pad">{error}</p>{/if}

  <footer>
    <span class="muted hint">Editing directly on the server.</span>
    <div class="actions">
      <button class="ghost" onclick={tryClose} disabled={saving}>Close</button>
      {#if readonlyReason === null}
        <button class="primary" onclick={save} disabled={saving || !dirty}>
          {saving ? "Saving…" : "Save"}
        </button>
      {/if}
    </div>
  </footer>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    border: none;
    border-radius: 0;
    cursor: default;
    z-index: 20;
  }

  .modal {
    position: fixed;
    inset: 5vh 5vw;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    z-index: 21;
    overflow: hidden;
  }

  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    background: var(--surface);
  }

  header {
    border-bottom: 1px solid var(--border);
  }

  footer {
    border-top: 1px solid var(--border);
  }

  .path {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    color: var(--warn, #fbbf24);
  }

  .pad {
    padding: 16px;
  }

  textarea {
    flex: 1;
    resize: none;
    border: none;
    border-radius: 0;
    background: var(--bg);
    padding: 14px 16px;
    font-family: ui-monospace, monospace;
    font-size: 13px;
    line-height: 1.5;
    tab-size: 2;
  }

  textarea:focus {
    outline: none;
  }

  .hint {
    font-size: 12px;
  }

  .actions {
    display: flex;
    gap: 10px;
  }

  .mono {
    font-family: ui-monospace, monospace;
  }
</style>
