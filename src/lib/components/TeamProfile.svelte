<script lang="ts">
  import { auth } from "../auth.svelte";
  import {
    getTeam,
    listDeploys,
    listIssues,
    listMembers,
    listProjects,
    updateTeam,
    type CloudProject,
    type DeployEntry,
    type Issue,
    type Team,
    type TeamMember,
  } from "../cloud";
  import ImagePicker from "./ImagePicker.svelte";
  import Markdown from "./Markdown.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  let {
    teamId,
    onBack,
    onUpdated,
    onOpenProfile,
    onOpenProject,
  }: {
    teamId: string;
    onBack: () => void;
    onUpdated?: (team: Team) => void;
    onOpenProfile?: (userId: string) => void;
    onOpenProject?: (projectId: string) => void;
  } = $props();

  let team = $state<Team | null>(null);
  let members = $state<TeamMember[]>([]);
  let projects = $state<CloudProject[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Aggregate statistics across the team's projects.
  let statsLoading = $state(false);
  let openIssues = $state(0);
  let deployTotal = $state(0);

  async function loadAggregate(projs: CloudProject[]) {
    statsLoading = true;
    try {
      const results = await Promise.all(
        projs.map(async (p) => {
          const [iss, dep] = await Promise.all([
            listIssues(p.id).catch(() => [] as Issue[]),
            listDeploys(p.id).catch(() => [] as DeployEntry[]),
          ]);
          return { open: iss.filter((i) => i.status === "open").length, deploys: dep.length };
        }),
      );
      openIssues = results.reduce((s, r) => s + r.open, 0);
      deployTotal = results.reduce((s, r) => s + r.deploys, 0);
    } finally {
      statsLoading = false;
    }
  }

  function memberName(m: TeamMember): string {
    return m.display_name?.trim() || m.username || "Unknown";
  }

  const isOwner = $derived(!!team && auth.user?.id === team.owner_id);

  // Edit buffer.
  let editing = $state(false);
  let saving = $state(false);
  let name = $state("");
  let location = $state("");
  let website = $state("");
  let logoUrl = $state("");
  let description = $state("");

  $effect(() => {
    const id = teamId;
    loading = true;
    error = null;
    editing = false;
    projects = [];
    openIssues = 0;
    deployTotal = 0;
    Promise.all([getTeam(id), listMembers(id).catch(() => [] as TeamMember[])])
      .then(([t, m]) => {
        team = t;
        members = m;
      })
      .catch((e) => (error = String(e instanceof Error ? e.message : e)))
      .finally(() => (loading = false));
    listProjects(id)
      .then((p) => {
        projects = p;
        void loadAggregate(p);
      })
      .catch(() => (projects = []));
  });

  const ownerName = $derived.by(() => {
    const owner = members.find((m) => m.role === "owner");
    return owner?.display_name?.trim() || owner?.username || null;
  });

  function startEdit() {
    if (!team) return;
    name = team.name;
    location = team.location ?? "";
    website = team.website ?? "";
    logoUrl = team.logo_url ?? "";
    description = team.description ?? "";
    error = null;
    editing = true;
  }

  async function save() {
    if (name.trim() === "") return;
    saving = true;
    error = null;
    try {
      const updated = await updateTeam(teamId, {
        name: name.trim(),
        location: location.trim() || null,
        website: website.trim() || null,
        logo_url: logoUrl.trim() || null,
        description: description.trim() || null,
      });
      team = updated;
      editing = false;
      onUpdated?.(updated);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      saving = false;
    }
  }

  function created(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "long" });
  }

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
    <p class="muted center">Loading team…</p>
  {:else if error && !team}
    <p class="error">{error}</p>
  {:else if team}
    {#if editing}
      <div class="card edit">
        <h2>Edit team</h2>
        <div class="field">
          <label for="t-name">Team name</label>
          <input id="t-name" bind:value={name} autocomplete="off" />
        </div>
        <div class="two">
          <div class="field">
            <label for="t-loc">Location</label>
            <input id="t-loc" bind:value={location} placeholder="e.g. Remote" autocomplete="off" />
          </div>
          <div class="field">
            <label for="t-web">Website</label>
            <input id="t-web" bind:value={website} placeholder="example.com" autocomplete="off" spellcheck="false" />
          </div>
        </div>
        <div class="field">
          <span class="field-label">Logo</span>
          <ImagePicker bind:value={logoUrl} kind="logo" owner={teamId} shape="square" />
        </div>
        <div class="field">
          <label for="t-desc">README <span class="muted">(Markdown)</span></label>
          <MarkdownEditor id="t-desc" bind:value={description} rows={8} placeholder="What is this team about? Headings, bold, lists, links…" />
        </div>
        {#if error}<p class="error">{error}</p>{/if}
        <div class="row-actions end">
          <button class="ghost" onclick={() => (editing = false)} disabled={saving}>Cancel</button>
          <button class="primary" onclick={save} disabled={saving || name.trim() === ""}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    {:else}
      <header class="head">
        <div class="identity">
          {#if team.logo_url}
            <img class="logo-img" src={team.logo_url} alt={team.name} />
          {:else}
            <span class="logo">{team.name.charAt(0).toUpperCase()}</span>
          {/if}
          <div class="names">
            <h1>{team.name}</h1>
            <span class="muted sub">Team{#if ownerName} · owned by {ownerName}{/if}</span>
          </div>
        </div>
        {#if isOwner}
          <button class="ghost" onclick={startEdit}>Edit team</button>
        {/if}
      </header>

      <div class="meta">
        {#if team.location}<span class="meta-item">{team.location}</span>{/if}
        {#if team.website && href(team.website)}
          <a class="meta-item link" href={href(team.website)} target="_blank" rel="noopener noreferrer">
            {hostOf(team.website)}
          </a>
        {/if}
        <span class="meta-item">{members.length} {members.length === 1 ? "member" : "members"}</span>
        <span class="meta-item muted">Created {created(team.created_at)}</span>
      </div>

      <div class="stats">
        <div class="stat">
          <span class="stat-num">{projects.length}</span>
          <span class="stat-label muted">{projects.length === 1 ? "Project" : "Projects"}</span>
        </div>
        <div class="stat">
          <span class="stat-num">{members.length}</span>
          <span class="stat-label muted">{members.length === 1 ? "Member" : "Members"}</span>
        </div>
        <div class="stat">
          <span class="stat-num">{statsLoading ? "…" : openIssues}</span>
          <span class="stat-label muted">Open {openIssues === 1 ? "issue" : "issues"}</span>
        </div>
        <div class="stat">
          <span class="stat-num">{statsLoading ? "…" : deployTotal}</span>
          <span class="stat-label muted">{deployTotal === 1 ? "Deploy" : "Deploys"}</span>
        </div>
      </div>

      <div class="card readme">
        <div class="card-head"><h2>README</h2></div>
        {#if team.description && team.description.trim() !== ""}
          <Markdown source={team.description} />
        {:else}
          <p class="muted">
            {isOwner ? "Add a README so your team knows what this is about." : "No README yet."}
          </p>
        {/if}
      </div>

      {#if members.length > 0}
        <div class="card">
          <div class="card-head"><h2>Members</h2></div>
          <div class="member-list">
            {#each members as m (m.user_id)}
              <button class="member" onclick={() => onOpenProfile?.(m.user_id)} title="View profile">
                {#if m.avatar_url}
                  <img class="avatar-img" src={m.avatar_url} alt="" />
                {:else}
                  <span class="avatar">{memberName(m).charAt(0).toUpperCase()}</span>
                {/if}
                <span class="m-name">{memberName(m)}</span>
                <span class="m-role role-{m.role}">{m.role}</span>
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

      {#if !isOwner}
        <p class="hint muted">Only the team owner can edit this page.</p>
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

  .logo,
  .logo-img {
    width: 72px;
    height: 72px;
    border-radius: 16px;
    flex-shrink: 0;
  }

  .logo {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-weight: 700;
    font-size: 30px;
  }

  .logo-img {
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

  .sub {
    font-size: 13px;
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

  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    margin-bottom: 22px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
  }

  .stat-num {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.1;
  }

  .stat-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px;
    margin-bottom: 16px;
  }

  .member-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .member {
    display: flex;
    align-items: center;
    gap: 10px;
    background: transparent;
    border: none;
    border-radius: 8px;
    padding: 6px 8px;
    text-align: left;
  }

  .member:hover {
    background: var(--surface-2);
  }

  .member .avatar,
  .member .avatar-img {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .member .avatar {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-weight: 700;
    font-size: 13px;
  }

  .member .avatar-img {
    object-fit: cover;
    border: 1px solid var(--border);
  }

  .m-name {
    flex: 1;
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .m-role {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .role-owner {
    color: #fbbf24;
  }

  .role-admin {
    color: #60a5fa;
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

  .card-head {
    margin-bottom: 12px;
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

  .hint {
    margin-top: 14px;
    font-size: 12px;
    text-align: center;
  }

  @media (max-width: 640px) {
    .two {
      grid-template-columns: 1fr;
    }

    .stats {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
