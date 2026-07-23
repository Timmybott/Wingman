<script lang="ts">
  import { onMount } from "svelte";
  import { createTeam, listTeams, type Team } from "../cloud";
  import { setActiveTeam } from "../team.svelte";
  import ImagePicker from "./ImagePicker.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  let { onReady }: { onReady: () => void } = $props();

  let teams = $state<Team[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // "pick" = choose an existing team; "create" = run the creation wizard.
  let mode = $state<"pick" | "create">("pick");
  const steps = ["Name", "Logo", "About"];
  let step = $state(0);
  let busy = $state(false);

  let name = $state("");
  let location = $state("");
  let website = $state("");
  let logoUrl = $state("");
  let description = $state("");
  // Stable id to scope the logo upload before the team exists.
  const draftId = crypto.randomUUID();

  onMount(async () => {
    try {
      teams = await listTeams();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  });

  function choose(team: Team) {
    setActiveTeam(team);
    onReady();
  }

  function startCreate() {
    mode = "create";
    step = 0;
    error = null;
  }

  function back() {
    if (step > 0) step -= 1;
    else mode = "pick";
  }

  function next() {
    if (step < steps.length - 1) step += 1;
  }

  async function create() {
    if (name.trim() === "") {
      step = 0;
      return;
    }
    busy = true;
    error = null;
    try {
      const team = await createTeam(name.trim(), {
        location: location.trim() || null,
        website: website.trim() || null,
        logo_url: logoUrl.trim() || null,
        description: description.trim() || null,
      });
      choose(team);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
      busy = false;
    }
  }
</script>

<div class="setup">
  {#if mode === "pick"}
    <h2>Choose a team</h2>
    <p class="muted">
      A team is where your panels, projects, deploy history and issues are
      shared. Pick a team you belong to, or create a new one.
    </p>

    {#if loading}
      <p class="muted">Loading…</p>
    {:else}
      {#if teams.length > 0}
        <div class="teams">
          {#each teams as team (team.id)}
            <button class="team" onclick={() => choose(team)}>
              {#if team.logo_url}
                <img class="t-logo" src={team.logo_url} alt="" />
              {:else}
                <span class="t-logo placeholder">{team.name.charAt(0).toUpperCase()}</span>
              {/if}
              <span class="name">{team.name}</span>
              <span class="muted go">Open →</span>
            </button>
          {/each}
        </div>
      {/if}
      <button class="primary create" onclick={startCreate}>Create a team</button>
    {/if}
  {:else}
    <div class="wiz-head">
      <h2>Create a team</h2>
      <ol class="stepper">
        {#each steps as label, i (label)}
          <li class:active={i === step} class:done={i < step}>
            <span class="num">{i + 1}</span>
            <span class="lbl">{label}</span>
          </li>
        {/each}
      </ol>
    </div>

    {#if step === 0}
      <div class="wiz-step">
        <div class="field">
          <label for="w-name">Team name</label>
          <input id="w-name" bind:value={name} placeholder="e.g. Acme Servers" autocomplete="off" disabled={busy} />
        </div>
        <div class="two">
          <div class="field">
            <label for="w-loc">Location <span class="muted">(optional)</span></label>
            <input id="w-loc" bind:value={location} placeholder="e.g. Remote" autocomplete="off" disabled={busy} />
          </div>
          <div class="field">
            <label for="w-web">Website <span class="muted">(optional)</span></label>
            <input id="w-web" bind:value={website} placeholder="example.com" autocomplete="off" spellcheck="false" disabled={busy} />
          </div>
        </div>
      </div>
    {:else if step === 1}
      <div class="wiz-step">
        <span class="field-label">Team logo <span class="muted">(optional)</span></span>
        <ImagePicker bind:value={logoUrl} kind="logo" owner={draftId} shape="square" />
      </div>
    {:else}
      <div class="wiz-step">
        <span class="field-label">README <span class="muted">(optional, Markdown)</span></span>
        <MarkdownEditor bind:value={description} rows={7} placeholder="What is this team about?" />
      </div>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}

    <div class="wiz-actions">
      <button class="ghost" onclick={back} disabled={busy}>← Back</button>
      {#if step < steps.length - 1}
        <button class="primary" onclick={next} disabled={busy || (step === 0 && name.trim() === "")}>
          Next →
        </button>
      {:else}
        <button class="primary" onclick={create} disabled={busy || name.trim() === ""}>
          {busy ? "Creating…" : "Create team"}
        </button>
      {/if}
    </div>
  {/if}

  {#if error && mode === "pick"}<p class="error">{error}</p>{/if}
</div>

<style>
  .setup {
    max-width: 480px;
    margin: 8vh auto 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
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

  .teams {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 16px;
  }

  .team {
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    padding: 10px 14px;
    text-align: left;
  }

  .team:hover {
    border-color: var(--accent);
  }

  .t-logo {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .t-logo.placeholder {
    display: grid;
    place-items: center;
    background: var(--surface);
    border: 1px solid var(--border);
    font-weight: 700;
  }

  .name {
    font-weight: 600;
    flex: 1;
  }

  .go {
    font-size: 12px;
  }

  .create {
    width: 100%;
  }

  .wiz-head {
    margin-bottom: 20px;
  }

  .stepper {
    list-style: none;
    display: flex;
    gap: 8px;
    margin: 14px 0 0;
    padding: 0;
  }

  .stepper li {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .stepper li + li::before {
    content: "";
    width: 16px;
    height: 1px;
    background: var(--border);
    margin-right: 2px;
  }

  .stepper .num {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    font-size: 11px;
  }

  .stepper li.active {
    color: var(--text);
  }

  .stepper li.active .num {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .stepper li.done .num {
    border-color: var(--accent);
    color: var(--accent);
  }

  .wiz-step {
    margin-bottom: 18px;
    min-height: 120px;
  }

  .field {
    margin-bottom: 14px;
  }

  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .wiz-actions {
    display: flex;
    justify-content: space-between;
    gap: 10px;
  }

  @media (max-width: 520px) {
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
