<script lang="ts">
  import { onMount } from "svelte";
  import { auth } from "../auth.svelte";
  import {
    inviteMember,
    listMembers,
    removeMember,
    setMemberRole,
    type TeamMember,
  } from "../cloud";

  let {
    teamId,
    onOpenProfile,
  }: {
    teamId: string;
    onOpenProfile: (userId: string) => void;
  } = $props();

  let members = $state<TeamMember[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let identifier = $state("");
  let inviting = $state(false);
  let info = $state<string | null>(null);
  let removingId = $state<string | null>(null);
  let roleBusyId = $state<string | null>(null);

  const currentUserId = $derived(auth.user?.id ?? null);
  const myRole = $derived(members.find((m) => m.user_id === currentUserId)?.role ?? null);
  const isAdmin = $derived(myRole === "owner" || myRole === "admin");
  const isOwner = $derived(myRole === "owner");

  async function load() {
    loading = true;
    error = null;
    try {
      members = await listMembers(teamId);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function invite(event: SubmitEvent) {
    event.preventDefault();
    if (identifier.trim() === "") return;
    inviting = true;
    error = null;
    info = null;
    try {
      await inviteMember(teamId, identifier);
      info = `${identifier.trim()} was added to the team.`;
      identifier = "";
      await load();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      inviting = false;
    }
  }

  async function remove(member: TeamMember) {
    removingId = member.user_id;
    error = null;
    info = null;
    try {
      await removeMember(teamId, member.user_id);
      await load();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      removingId = null;
    }
  }

  /** Owner-only: promote a member to admin or demote them back to member. */
  async function changeRole(member: TeamMember, role: "admin" | "member") {
    roleBusyId = member.user_id;
    error = null;
    info = null;
    try {
      await setMemberRole(teamId, member.user_id, role);
      await load();
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      roleBusyId = null;
    }
  }

  function displayName(m: TeamMember): string {
    return m.display_name?.trim() || m.username || "Unknown";
  }
</script>

<div class="members">
  <div class="head">
    <h2>Members</h2>
    <p class="muted">Everyone on this team shares its projects, panels, history and issues.</p>
  </div>

  {#if isAdmin}
    <form onsubmit={invite}>
      <input
        type="text"
        bind:value={identifier}
        placeholder="Email or username"
        autocomplete="off"
        spellcheck="false"
        disabled={inviting}
      />
      <button type="submit" class="primary" disabled={inviting || identifier.trim() === ""}>
        {inviting ? "Adding…" : "Add member"}
      </button>
    </form>
    <p class="hint muted">Enter a teammate's email address or their username — they need a Feather account already.</p>
  {/if}

  {#if error}<p class="error">{error}</p>{:else if info}<p class="ok">{info}</p>{/if}

  {#if loading}
    <p class="muted center">Loading members…</p>
  {:else}
    <ul class="list">
      {#each members as member (member.user_id)}
        <li>
          <button class="who" onclick={() => onOpenProfile(member.user_id)} title="View profile">
            {#if member.avatar_url}
              <img class="avatar-img" src={member.avatar_url} alt="" />
            {:else}
              <span class="avatar">{displayName(member).charAt(0).toUpperCase()}</span>
            {/if}
            <div class="names">
              <span class="name">
                {displayName(member)}
                {#if member.user_id === currentUserId}<span class="you muted">(you)</span>{/if}
              </span>
              {#if member.username}<span class="muted handle">@{member.username}</span>{/if}
            </div>
          </button>
          <div class="right">
            <span class="role role-{member.role}">{member.role}</span>
            {#if isOwner && member.role === "member"}
              <button
                class="ghost"
                disabled={roleBusyId !== null}
                onclick={() => changeRole(member, "admin")}
                title="Grant admin rights"
              >
                {roleBusyId === member.user_id ? "…" : "Make admin"}
              </button>
            {:else if isOwner && member.role === "admin"}
              <button
                class="ghost"
                disabled={roleBusyId !== null}
                onclick={() => changeRole(member, "member")}
                title="Revoke admin rights"
              >
                {roleBusyId === member.user_id ? "…" : "Remove admin"}
              </button>
            {/if}
            {#if isAdmin && member.role !== "owner" && member.user_id !== currentUserId}
              <button
                class="ghost danger"
                disabled={removingId !== null}
                onclick={() => remove(member)}
              >
                {removingId === member.user_id ? "…" : "Remove"}
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .members {
    max-width: 620px;
    margin: 24px auto 0;
  }

  .head {
    margin-bottom: 18px;
  }

  h2 {
    font-size: 20px;
    margin-bottom: 6px;
  }

  p {
    margin: 0;
    line-height: 1.5;
  }

  form {
    display: flex;
    gap: 8px;
  }

  form input {
    flex: 1;
  }

  .hint {
    font-size: 12px;
    margin: 6px 0 0;
  }

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 18px;
  }

  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 10px 14px;
  }

  .who {
    display: flex;
    align-items: center;
    gap: 12px;
    background: transparent;
    border: none;
    border-radius: 8px;
    padding: 4px;
    margin: -4px;
    text-align: left;
    min-width: 0;
  }

  .who:hover .name {
    color: var(--accent);
  }

  .avatar,
  .avatar-img {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .avatar {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-weight: 700;
    font-size: 14px;
  }

  .avatar-img {
    object-fit: cover;
    border: 1px solid var(--border);
  }

  .names {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .name {
    font-weight: 600;
    font-size: 14px;
  }

  .you {
    font-weight: 400;
    font-size: 12px;
  }

  .handle {
    font-size: 12px;
  }

  .right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .role {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-radius: 20px;
    padding: 3px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }

  .role-owner {
    color: #fbbf24;
    border-color: #fbbf2455;
  }

  .role-admin {
    color: #60a5fa;
    border-color: #60a5fa55;
  }

  .danger:hover {
    color: var(--danger, #f87171);
    border-color: var(--danger, #f87171);
  }
</style>
