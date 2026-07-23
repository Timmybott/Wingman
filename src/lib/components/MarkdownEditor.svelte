<script lang="ts">
  import { tick } from "svelte";

  let {
    value = $bindable(""),
    placeholder = "",
    rows = 6,
    id,
  }: {
    value?: string;
    placeholder?: string;
    rows?: number;
    id?: string;
  } = $props();

  let ta: HTMLTextAreaElement;

  async function select(from: number, to: number) {
    await tick();
    ta.focus();
    ta.setSelectionRange(from, to);
  }

  /** Wrap the selection (or a placeholder) in `before`…`after`. */
  function surround(before: string, after: string, ph: string) {
    const s = ta.selectionStart;
    const e = ta.selectionEnd;
    const inner = value.slice(s, e) || ph;
    value = value.slice(0, s) + before + inner + after + value.slice(e);
    const pos = s + before.length;
    void select(pos, pos + inner.length);
  }

  /** Prefix every selected line (list items, heading, quote). */
  function linePrefix(prefix: string, numbered = false) {
    const s = ta.selectionStart;
    const e = ta.selectionEnd;
    const lineStart = value.lastIndexOf("\n", s - 1) + 1;
    const block = value.slice(lineStart, e) || "";
    const prefixed = block
      .split("\n")
      .map((l, i) => (numbered ? `${i + 1}. ` : prefix) + l)
      .join("\n");
    value = value.slice(0, lineStart) + prefixed + value.slice(e);
    void select(lineStart, lineStart + prefixed.length);
  }

  function link() {
    const s = ta.selectionStart;
    const e = ta.selectionEnd;
    const text = value.slice(s, e) || "text";
    value = value.slice(0, s) + `[${text}](url)` + value.slice(e);
    const urlStart = s + text.length + 3;
    void select(urlStart, urlStart + 3);
  }
</script>

<div class="md-editor">
  <div class="toolbar">
    <button type="button" title="Bold" onclick={() => surround("**", "**", "bold")}><b>B</b></button>
    <button type="button" class="ital" title="Italic" onclick={() => surround("*", "*", "italic")}><i>I</i></button>
    <button type="button" title="Heading" onclick={() => linePrefix("## ")}>H</button>
    <button type="button" title="Bulleted list" onclick={() => linePrefix("- ")}>List</button>
    <button type="button" title="Numbered list" onclick={() => linePrefix("", true)}>1.</button>
    <button type="button" title="Quote" onclick={() => linePrefix("> ")}>Quote</button>
    <button type="button" class="mono" title="Inline code" onclick={() => surround("`", "`", "code")}>Code</button>
    <button type="button" title="Link" onclick={link}>Link</button>
  </div>
  <textarea {id} bind:this={ta} bind:value {placeholder} {rows}></textarea>
</div>

<style>
  .md-editor {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg);
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .md-editor:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 5px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .toolbar button {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 9px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-muted);
    min-width: 30px;
  }

  .toolbar button:hover:not(:disabled) {
    background: var(--surface);
    color: var(--text);
    border-color: transparent;
  }

  .toolbar .ital {
    font-style: italic;
  }

  .toolbar .mono {
    font-family: ui-monospace, monospace;
  }

  .md-editor textarea {
    border: none;
    border-radius: 0;
    background: var(--bg);
  }

  .md-editor textarea:focus {
    box-shadow: none;
    border: none;
  }
</style>
