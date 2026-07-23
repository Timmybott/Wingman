<script lang="ts">
  import { uploadImage } from "../cloud";

  let {
    value = $bindable(""),
    kind,
    owner,
    shape = "square",
  }: {
    /** Bound image URL (empty = none). */
    value?: string;
    kind: "avatar" | "logo";
    /** Id used to scope the upload path (user/team/project). */
    owner: string;
    shape?: "square" | "circle";
  } = $props();

  let uploading = $state(false);
  let error = $state<string | null>(null);
  let fileInput: HTMLInputElement;

  async function onPick(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      error = "Please choose an image file.";
      return;
    }
    if (file.size > 5_000_000) {
      error = "Image is too large — 5 MB max.";
      return;
    }
    uploading = true;
    error = null;
    try {
      value = await uploadImage(kind, owner, file);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      uploading = false;
      if (fileInput) fileInput.value = "";
    }
  }
</script>

<div class="image-picker">
  {#if value}
    <img class="preview {shape}" src={value} alt="" />
  {:else}
    <div class="preview {shape} empty">No image</div>
  {/if}
  <div class="controls">
    <div class="row">
      <button type="button" class="ghost pick" onclick={() => fileInput.click()} disabled={uploading}>
        {uploading ? "Uploading…" : value ? "Change…" : "Choose file…"}
      </button>
      {#if value}
        <button type="button" class="ghost" onclick={() => (value = "")} disabled={uploading}>Remove</button>
      {/if}
    </div>
    {#if error}<span class="error small">{error}</span>{/if}
    <span class="hint muted">PNG, JPG or GIF, up to 5 MB.</span>
  </div>
  <input bind:this={fileInput} type="file" accept="image/*" onchange={onPick} hidden />
</div>

<style>
  .image-picker {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .preview {
    width: 72px;
    height: 72px;
    flex-shrink: 0;
    object-fit: cover;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }

  .preview.square {
    border-radius: 14px;
  }

  .preview.circle {
    border-radius: 50%;
  }

  .preview.empty {
    display: grid;
    place-items: center;
    color: var(--text-muted);
    font-size: 11px;
    text-align: center;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 5px;
    align-items: flex-start;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .pick {
    border: 1px solid var(--border);
  }

  .small {
    font-size: 12px;
  }

  .hint {
    font-size: 11px;
  }
</style>
