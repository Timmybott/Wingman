<script lang="ts">
  import { lineDiff, type LineOp } from "../linediff";

  let {
    path,
    oldText,
    newText,
    loading = false,
    error = null,
    onClose,
  }: {
    path: string;
    oldText: string;
    newText: string;
    loading?: boolean;
    error?: string | null;
    onClose: () => void;
  } = $props();

  const ops = $derived<LineOp[]>(loading || error ? [] : lineDiff(oldText, newText));
  const sign = { same: " ", add: "+", del: "−" } as const;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<button class="backdrop" aria-label="Close diff" onclick={onClose}></button>
<div class="modal" role="dialog" aria-modal="true" aria-label="File diff">
  <header>
    <span class="path mono">{path}</span>
    <button class="ghost" onclick={onClose} title="Close (Esc)">✕</button>
  </header>

  {#if loading}
    <p class="muted pad">Loading diff…</p>
  {:else if error}
    <p class="error pad">{error}</p>
  {:else if ops.length === 0}
    <p class="muted pad">No differences.</p>
  {:else}
    <div class="code">
      {#each ops as op, i (i)}
        <div class="line {op.type}"><span class="gutter">{sign[op.type]}</span><span class="text">{op.text || " "}</span></div>
      {/each}
    </div>
  {/if}
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

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .path {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pad {
    padding: 16px;
  }

  .code {
    flex: 1;
    overflow: auto;
    font-family: ui-monospace, monospace;
    font-size: 12.5px;
    line-height: 1.5;
    padding: 6px 0;
  }

  .line {
    display: flex;
    white-space: pre;
  }

  .gutter {
    flex-shrink: 0;
    width: 22px;
    text-align: center;
    color: var(--text-muted);
    user-select: none;
  }

  .text {
    padding-right: 16px;
  }

  .line.add {
    background: #10b98118;
  }

  .line.add .gutter {
    color: #34d399;
  }

  .line.del {
    background: #ef444418;
  }

  .line.del .gutter {
    color: #f87171;
  }

  .mono {
    font-family: ui-monospace, monospace;
  }
</style>
