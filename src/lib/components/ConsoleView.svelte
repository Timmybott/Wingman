<script lang="ts">
  import { stripAnsi } from "../format";
  import type { LiveState, Server } from "../types";

  let {
    server,
    live,
    lines,
    onSend,
    onClose,
  }: {
    server: Server;
    live: LiveState;
    lines: string[];
    onSend: (command: string) => Promise<void>;
    onClose: () => void;
  } = $props();

  let input = $state("");
  let sendError = $state<string | null>(null);
  let output = $state<HTMLElement | undefined>();
  let pinned = $state(true);

  const rendered = $derived(lines.map(stripAnsi));

  // Follow the output unless the user scrolled up to read.
  $effect(() => {
    void rendered.length;
    if (pinned && output) {
      output.scrollTop = output.scrollHeight;
    }
  });

  function onScroll() {
    if (!output) return;
    pinned = output.scrollTop + output.clientHeight >= output.scrollHeight - 24;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const command = input.trim();
    if (!command) return;
    input = "";
    sendError = null;
    try {
      await onSend(command);
    } catch (e) {
      sendError = String(e);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="console">
  <header>
    <button class="back ghost" onclick={onClose} title="Back (Esc)">← Back</button>
    <div class="title">
      <span class="dot {live.connected ? 'online' : 'offline'}"></span>
      <h3>{server.name}</h3>
      <span class="muted">{live.state ?? "…"}</span>
    </div>
  </header>

  <div class="output" bind:this={output} onscroll={onScroll}>
    {#if rendered.length === 0}
      <p class="muted">No output yet.</p>
    {:else}
      {#each rendered as line, i (i)}
        <div class="line">{line}</div>
      {/each}
    {/if}
  </div>

  <form onsubmit={submit}>
    <input
      bind:value={input}
      placeholder={live.connected ? "Type a command…" : "Not connected"}
      disabled={!live.connected}
      spellcheck="false"
      autocomplete="off"
    />
    <button class="primary" type="submit" disabled={!live.connected || input.trim() === ""}>
      Send
    </button>
  </form>
  {#if sendError}
    <p class="error send-error">{sendError}</p>
  {/if}
</div>

<style>
  .console {
    /* Full-page in-flow view that fills the main content area, not a drawer. */
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);
    min-height: 380px;
    max-width: 1000px;
    margin: 0 auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .back {
    flex-shrink: 0;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  h3 {
    font-size: 14px;
  }

  .output {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
    font-family: "Cascadia Code", "Fira Code", ui-monospace, monospace;
    font-size: 12.5px;
    line-height: 1.55;
    user-select: text;
  }

  .line {
    white-space: pre-wrap;
    word-break: break-word;
  }

  form {
    display: flex;
    gap: 8px;
    padding: 12px 16px;
    background: var(--surface);
    border-top: 1px solid var(--border);
  }

  .send-error {
    margin: 0;
    padding: 6px 16px 10px;
    background: var(--surface);
    font-size: 12px;
  }
</style>
