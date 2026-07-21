<script lang="ts">
  import { signIn, signUp } from "../auth.svelte";
  import Logo from "./Logo.svelte";

  let mode = $state<"login" | "signup">("login");
  let email = $state("");
  let password = $state("");
  let displayName = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let info = $state<string | null>(null);

  const canSubmit = $derived(
    email.trim() !== "" && password !== "" && !busy && (mode === "login" || displayName.trim() !== ""),
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    info = null;
    try {
      if (mode === "login") {
        await signIn(email.trim(), password);
        // On success the auth store updates and the app advances on its own.
      } else {
        const { needsConfirmation } = await signUp(email.trim(), password, displayName.trim());
        if (needsConfirmation) {
          info = "Account created — check your email to confirm, then sign in.";
          mode = "login";
        }
      }
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="auth">
  <div class="brand">
    <Logo size={40} />
    <h1>Feather</h1>
  </div>
  <p class="muted tagline">Deploy, plan and collaborate on your Pterodactyl projects.</p>

  <form onsubmit={submit}>
    {#if mode === "signup"}
      <div class="field">
        <label for="name">Display name</label>
        <input id="name" bind:value={displayName} autocomplete="off" />
      </div>
    {/if}
    <div class="field">
      <label for="email">Email</label>
      <input id="email" type="email" bind:value={email} autocomplete="email" spellcheck="false" />
    </div>
    <div class="field">
      <label for="password">Password</label>
      <input id="password" type="password" bind:value={password} autocomplete="current-password" />
    </div>

    {#if error}
      <p class="error">{error}</p>
    {:else if info}
      <p class="ok">{info}</p>
    {/if}

    <button type="submit" class="primary block" disabled={!canSubmit}>
      {busy ? "…" : mode === "login" ? "Sign in" : "Create account"}
    </button>
  </form>

  <p class="switch muted">
    {#if mode === "login"}
      No account?
      <button class="link" onclick={() => ((mode = "signup"), (error = null), (info = null))}>
        Create one
      </button>
    {:else}
      Already have an account?
      <button class="link" onclick={() => ((mode = "login"), (error = null), (info = null))}>
        Sign in
      </button>
    {/if}
  </p>
</div>

<style>
  .auth {
    max-width: 380px;
    margin: 8vh auto 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 30px;
    text-align: center;
  }

  .brand {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }

  h1 {
    font-size: 24px;
  }

  .tagline {
    margin: 8px 0 22px;
    line-height: 1.5;
  }

  form {
    text-align: left;
  }

  .field {
    margin-bottom: 14px;
  }

  .block {
    width: 100%;
    margin-top: 6px;
  }

  .switch {
    margin-top: 18px;
    font-size: 13px;
  }

  .link {
    background: none;
    border: none;
    color: var(--accent);
    padding: 0;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
