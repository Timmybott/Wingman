<script lang="ts">
  import { auth } from "../auth.svelte";
  import {
    getProfile,
    listUserProjects,
    listUserTeams,
    updateMyProfile,
    type CloudProject,
    type Team,
    type UserProfile,
  } from "../cloud";
  import ImagePicker from "./ImagePicker.svelte";
  import Markdown from "./Markdown.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  let {
    userId,
    onBack,
    onOpenTeam,
    onOpenProject,
  }: {
    userId: string;
    onBack: () => void;
    onOpenTeam?: (teamId: string) => void;
    onOpenProject?: (projectId: string) => void;
  } = $props();

  let profile = $state<UserProfile | null>(null);
  let teams = $state<Team[]>([]);
  let projects = $state<CloudProject[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const isMe = $derived(auth.user?.id === userId);

  // Edit buffer.
  let editing = $state(false);
  let saving = $state(false);
  let displayName = $state("");
  let location = $state("");
  let website = $state("");
  let avatarUrl = $state("");
  let bio = $state("");

  $effect(() => {
    const id = userId;
    loading = true;
    error = null;
    editing = false;
    teams = [];
    projects = [];
    getProfile(id)
      .then((p) => (profile = p))
      .catch((e) => (error = String(e instanceof Error ? e.message : e)))
      .finally(() => (loading = false));
    listUserTeams(id)
      .then((t) => (teams = t))
      .catch(() => (teams = []));
    listUserProjects(id)
      .then((p) => (projects = p))
      .catch(() => (projects = []));
  });

  const name = $derived(profile?.display_name?.trim() || profile?.username || "Unknown");

  function startEdit() {
    if (!profile) return;
    displayName = profile.display_name ?? "";
    location = profile.location ?? "";
    website = profile.website ?? "";
    avatarUrl = profile.avatar_url ?? "";
    bio = profile.bio ?? "";
    error = null;
    editing = true;
  }

  async function save() {
    saving = true;
    error = null;
    try {
      profile = await updateMyProfile({
        display_name: displayName.trim() || null,
        location: location.trim() || null,
        website: website.trim() || null,
        avatar_url: avatarUrl.trim() || null,
        bio: bio.trim() || null,
      });
      editing = false;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      saving = false;
    }
  }

  function joined(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "long" });
  }

  /** Normalise a user-entered website into a safe, clickable href. */
  function href(url: string): string | null {
    const t = url.trim();
    if (t === "") return null;
    const full = /^https?:\/\//i.test(t) ? t : `https://${t}`;
    try {
      const u = new URL(full);
      return u.protocol === "http:" || u.protocol === "https:" ? u.href : null;
    } catch {
      return null;
    }
  }

  function hostOf(url: string): string {
    try {
      return new URL(href(url) ?? "").host;
    } catch {
      return url;
    }
  }
</script>

<div class="profile">
  <button class="back ghost" onclick={onBack}>← Back</button>

  {#if loading}
    <p class="muted center">Loading profile…</p>
  {:else if error && !profile}
    <p class="error">{error}</p>
  {:else if profile}
    {#if editing}
      <div class="card edit">
        <h2>Edit your profile</h2>
        <div class="field">
          <label for="p-name">Display name</label>
          <input id="p-name" bind:value={displayName} autocomplete="off" />
        </div>
        <div class="two">
          <div class="field">
            <label for="p-loc">Location</label>
            <input id="p-loc" bind:value={location} placeholder="e.g. Berlin" autocomplete="off" />
          </div>
          <div class="field">
            <label for="p-web">Website</label>
            <input id="p-web" bind:value={website} placeholder="example.com" autocomplete="off" spellcheck="false" />
          </div>
        </div>
        <div class="field">
          <span class="field-label">Avatar</span>
          <ImagePicker bind:value={avatarUrl} kind="avatar" owner={userId} shape="circle" />
        </div>
        <div class="field">
          <label for="p-bio">README <span class="muted">(Markdown)</span></label>
          <MarkdownEditor id="p-bio" bind:value={bio} rows={8} placeholder="Tell your team about yourself — headings, bold, lists, links…" />
        </div>
        {#if error}<p class="error">{error}</p>{/if}
        <div class="row-actions end">
          <button class="ghost" onclick={() => (editing = false)} disabled={saving}>Cancel</button>
          <button class="primary" onclick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
        </div>
      </div>
    {:else}
      <header class="head">
        <div class="identity">
          {#if profile.avatar_url}
            <img class="avatar-img" src={profile.avatar_url} alt={name} />
          {:else}
            <span class="avatar">{name.charAt(0).toUpperCase()}</span>
          {/if}
          <div class="names">
            <h1>{name}</h1>
            {#if profile.username}<span class="muted handle">@{profile.username}</span>{/if}
          </div>
        </div>
        {#if isMe}
          <button class="ghost" onclick={startEdit}>Edit profile</button>
        {/if}
      </header>

      <div class="meta">
        {#if profile.location}<span class="meta-item">{profile.location}</span>{/if}
        {#if profile.website && href(profile.website)}
          <a class="meta-item link" href={href(profile.website)} target="_blank" rel="noopener noreferrer">
            {hostOf(profile.website)}
          </a>
        {/if}
        <span class="meta-item muted">Joined {joined(profile.created_at)}</span>
      </div>

      <div class="card readme">
        <div class="card-head"><h2>README</h2></div>
        {#if profile.bio && profile.bio.trim() !== ""}
          <Markdown source={profile.bio} />
        {:else}
          <p class="muted">
            {isMe ? "Add a README to introduce yourself to your team." : "No README yet."}
          </p>
        {/if}
      </div>

      {#if teams.length > 0}
        <div class="card">
          <div class="card-head"><h2>Teams</h2></div>
          <div class="chips">
            {#each teams as t (t.id)}
              <button class="chip" onclick={() => onOpenTeam?.(t.id)} title="Open team">
                {#if t.logo_url}<img class="chip-logo" src={t.logo_url} alt="" />{/if}
                {t.name}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if projects.length > 0}
        <div class="card">
          <div class="card-head"><h2>Projects</h2></div>
          <div class="chips">
            {#each projects as p (p.id)}
              <button class="chip" onclick={() => onOpenProject?.(p.id)} title="Open project">
                {#if p.logo_url}<img class="chip-logo" src={p.logo_url} alt="" />{/if}
                {p.name}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .profile {
    max-width: 760px;
    margin: 22px auto 0;
  }

  .back {
    margin-bottom: 14px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 12px;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 0;
  }

  .avatar,
  .avatar-img {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .avatar {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-weight: 700;
    font-size: 30px;
  }

  .avatar-img {
    object-fit: cover;
    border: 1px solid var(--border);
  }

  .names {
    min-width: 0;
  }

  h1 {
    font-size: 24px;
    margin-bottom: 2px;
  }

  .handle {
    font-size: 14px;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-bottom: 22px;
    font-size: 13px;
  }

  .meta-item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .link {
    color: var(--accent);
  }

  .link:hover {
    text-decoration: underline;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px;
    margin-bottom: 16px;
  }

  .card-head {
    margin-bottom: 12px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 5px 12px;
    font-size: 13px;
    font-weight: 600;
  }

  .chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .chip-logo {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    object-fit: cover;
  }

  h2 {
    font-size: 14px;
  }

  .field {
    margin-bottom: 14px;
  }

  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  .edit h2 {
    margin-bottom: 16px;
    font-size: 16px;
  }

  .row-actions {
    display: flex;
    gap: 10px;
  }

  .row-actions.end {
    justify-content: flex-end;
    margin-top: 8px;
  }

  @media (max-width: 640px) {
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
