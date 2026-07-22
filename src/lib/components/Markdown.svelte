<script lang="ts">
  import { renderMarkdown } from "../markdown";

  let {
    source,
    onToggleTask,
  }: {
    source: string;
    /** When set, task checkboxes are interactive and report toggles by index. */
    onToggleTask?: (index: number, checked: boolean) => void;
  } = $props();

  const html = $derived(renderMarkdown(source));

  function onClick(event: MouseEvent) {
    const el = event.target;
    if (
      el instanceof HTMLInputElement &&
      el.type === "checkbox" &&
      el.dataset.task !== undefined
    ) {
      if (!onToggleTask) {
        // Read-only: don't let the box flip.
        event.preventDefault();
        return;
      }
      onToggleTask(Number(el.dataset.task), el.checked);
    }
  }
</script>

<!-- Safe: renderMarkdown escapes all input and emits only its own tag set. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="md" onclick={onClick}>{@html html}</div>

<style>
  .md {
    line-height: 1.65;
    word-break: break-word;
  }

  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4),
  .md :global(h5),
  .md :global(h6) {
    margin: 1.1em 0 0.5em;
    line-height: 1.3;
  }

  .md :global(h1) {
    font-size: 1.4em;
  }
  .md :global(h2) {
    font-size: 1.25em;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.2em;
  }
  .md :global(h3) {
    font-size: 1.1em;
  }

  .md :global(p) {
    margin: 0 0 0.8em;
  }

  .md :global(:first-child) {
    margin-top: 0;
  }
  .md :global(:last-child) {
    margin-bottom: 0;
  }

  .md :global(ul),
  .md :global(ol) {
    margin: 0 0 0.8em;
    padding-left: 1.5em;
  }

  .md :global(ul.task-list) {
    list-style: none;
    padding-left: 0.2em;
  }

  .md :global(li.task) {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .md :global(li.task input) {
    width: auto;
    margin-top: 4px;
  }

  .md :global(li) {
    margin: 0.2em 0;
  }

  .md :global(a) {
    color: var(--accent);
  }

  .md :global(a:hover) {
    text-decoration: underline;
  }

  .md :global(code) {
    background: var(--surface-2);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.9em;
    font-family: ui-monospace, monospace;
  }

  .md :global(pre) {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 14px;
    overflow-x: auto;
    margin: 0 0 0.8em;
  }

  .md :global(pre code) {
    background: none;
    padding: 0;
    font-size: 0.85em;
  }

  .md :global(blockquote) {
    margin: 0 0 0.8em;
    padding: 2px 14px;
    border-left: 3px solid var(--border);
    color: var(--text-muted);
  }

  .md :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1.2em 0;
  }

  .md :global(strong) {
    font-weight: 700;
  }
</style>
