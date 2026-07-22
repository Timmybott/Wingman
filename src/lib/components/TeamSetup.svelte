<script lang="ts">
  import { onMount } from "svelte";
  import { createTeam, listTeams, type Team } from "../cloud";
  import { setActiveTeam } from "../team.svelte";

  let { onReady }: { onReady: () => void } = $props();

  let teams = $state<Team[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let newName = $state("");
  let busy = $state(false);

  // Optional profile fields, revealed on demand.
  let showDetails = $state(false);
  let location = $state("");
  let website = $state("");
  let logoUrl = $state("");
  let description = $state("");

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

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (newName.trim() === "") return;
    busy = true;
    error = null;
    try {
      const team = await createTeam(newName, {
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
  <h2>Choose a team</h2>
  <p class="muted">
    A team is where your panels, projects, deploy history and issues are shared.
    Create one, or pick an existing team you belong to.
  </p>

  {#if loading}
    <p class="muted">Loading…</p>
  {:else}
    {#if teams.length > 0}
      <div class="teams">
        {#each teams as team (team.id)}
          <button class="team" onclick={() => choose(team)}>
            <span class="name">{team.name}</span>
            <span class="muted">Open →</span>
          </button>
        {/each}
      </div>
      <div class="divider"><span class="muted">or create a new team</span></div>
    {/if}

    <form onsubmit={create}>
      <div class="row">
        <input bind:value={newName} placeholder="New team name" autocomplete="off" disabled={busy} />
        <button type="submit" class="primary" disabled={busy || newName.trim() === ""}>Create</button>
      </div>

      <button type="button" class="details-toggle muted" onclick={() => (showDetails = !showDetails)}>
        {showDetails ? "▾" : "▸"} Add details (optional)
      </button>

      {#if showDetails}
        <div class="details">
          <div class="two">
            <input bind:value={location} placeholder="Location" autocomplete="off" disabled={busy} />
            <input bind:value={website} placeholder="Website" autocomplete="off" spellcheck="false" disabled={busy} />
          </div>
          <input bind:value={logoUrl} placeholder="Logo image URL" autocomplete="off" spellcheck="false" disabled={busy} />
          <textarea bind:value={description} rows="4" placeholder="README (Markdown) — what is this team about?" disabled={busy}></textarea>
        </div>
      {/if}
    </form>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .setup {
    max-width: 460px;
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
    justify-content: space-between;
    background: var(--surface-2);
    border: 1px solid var(--border);
    padding: 12px 14px;
    text-align: left;
  }

  .team:hover {
    border-color: var(--accent);
  }

  .name {
    font-weight: 600;
  }

  .divider {
    text-align: center;
    font-size: 12px;
    margin: 8px 0 16px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .row input {
    flex: 1;
  }

  .details-toggle {
    align-self: flex-start;
    background: transparent;
    border: none;
    padding: 0;
    font-size: 12px;
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
  }
</style>
