<script lang="ts">
  import Logo from "./Logo.svelte";

  let {
    userEmail,
    teamName,
    onOpenProfile,
    onOpenTeam,
    onSwitchTeam,
    onLogout,
  }: {
    userEmail: string;
    teamName: string;
    onOpenProfile: () => void;
    onOpenTeam: () => void;
    onSwitchTeam: () => void;
    onLogout: () => void;
  } = $props();

  let menuOpen = $state(false);
</script>

<header>
  <div class="brand">
    <Logo size={26} />
    <h1>Feather</h1>
  </div>

  <div class="right">
    <div class="account">
      <button class="team" onclick={() => (menuOpen = !menuOpen)} title={userEmail}>
        <span class="team-name">{teamName}</span>
        <span class="chevron">▾</span>
      </button>
      {#if menuOpen}
        <button class="backdrop" aria-label="Close menu" onclick={() => (menuOpen = false)}></button>
        <div class="menu">
          <div class="menu-user muted">{userEmail}</div>
          <button
            class="menu-item"
            onclick={() => {
              menuOpen = false;
              onOpenProfile();
            }}
          >
            Your profile
          </button>
          <button
            class="menu-item"
            onclick={() => {
              menuOpen = false;
              onOpenTeam();
            }}
          >
            Team profile
          </button>
          <button
            class="menu-item"
            onclick={() => {
              menuOpen = false;
              onSwitchTeam();
            }}
          >
            Switch team
          </button>
          <button
            class="menu-item"
            onclick={() => {
              menuOpen = false;
              onLogout();
            }}
          >
            Log out
          </button>
        </div>
      {/if}
    </div>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  h1 {
    font-size: 15px;
  }

  .right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .account {
    position: relative;
  }

  .team {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 5px 10px;
  }

  .team:hover {
    border-color: var(--accent);
  }

  .team-name {
    font-weight: 600;
    font-size: 13px;
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    color: var(--text-muted);
    font-size: 10px;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    z-index: 30;
    cursor: default;
  }

  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    min-width: 180px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px;
    z-index: 31;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }

  .menu-user {
    padding: 6px 10px;
    font-size: 12px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu-item {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 13px;
  }

  .menu-item:hover {
    background: var(--surface-2);
  }
</style>
