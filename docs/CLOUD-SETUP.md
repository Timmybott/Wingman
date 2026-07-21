# Feather cloud setup (Supabase)

Feather 3.x adds accounts and team collaboration on top of the desktop app.
Accounts, teams, saved Pterodactyl panels, projects, deploy history and issues
live in a free [Supabase](https://supabase.com) project. The deploy engine
still runs locally on each teammate's machine — Supabase only holds the shared
data.

You only need to do this once. It takes about five minutes.

## 1. Create the project

1. Sign up at [supabase.com](https://supabase.com) (free) and click **New project**.
2. Pick a name and a strong database password (you won't need the password for
   Feather — keep it somewhere safe anyway). Choose the region closest to your
   team.
3. Wait ~2 minutes for it to provision.

## 2. Create the encryption secret

Panel API keys are stored encrypted. Feather reads the encryption key from
Supabase Vault. Create it once:

1. Go to **SQL Editor → New query**, paste this, and run it. Replace the long
   string with your own random value (any 40+ random characters):

   ```sql
   select vault.create_secret(
     'CHANGE-ME-to-a-long-random-string-4f9a2b7c8d1e', -- the actual secret
     'feather_encryption_key'                          -- its name (keep exactly this)
   );
   ```

   Losing this secret makes stored panel keys unrecoverable — you'd just
   re-enter them. Never share it.

## 3. Create the tables

In **SQL Editor → New query**, paste the entire contents of
[`supabase/0001_foundation.sql`](../supabase/0001_foundation.sql) and run it.
It creates the profiles, teams, panels and projects tables with all the
security rules. You should see "Success. No rows returned".

## 4. Turn on email login

1. Go to **Authentication → Providers → Email** and make sure it's enabled.
2. For easy testing you can turn **Confirm email** off (Authentication →
   Providers → Email → "Confirm email"). You can turn it back on later.

## 5. Send me two values

Go to **Project Settings → API** and copy:

- **Project URL** (looks like `https://abcdefgh.supabase.co`)
- **anon public** key (the long `eyJ...` token labelled *anon* / *public*)

Send me **those two**. They are meant to be used inside the app and are
protected by the security rules above.

> ⚠️ Do **not** send the **service_role** key or the database password — those
> are admin secrets that must never go into the app or a chat.

Once I have the Project URL and anon key, I'll wire up login, team creation and
the encrypted multi-panel storage, and we continue milestone by milestone.
