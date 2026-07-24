# Feather cloud setup (Supabase)

Feather's team features (accounts, teams, members, encrypted shared panels,
projects, deploy history and issues) run on a free
[Supabase](https://supabase.com) project. The deploy engine still runs locally
on each teammate's machine — Supabase only holds the shared data.

You only need to do this once (one person per team), and it takes about five
minutes. Afterwards, share the Project URL and anon key with your teammates so
they can point their Feather at the same backend.

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

Then open a **new query**, paste the entire contents of
[`supabase/0002_team_create_rpc.sql`](../supabase/0002_team_create_rpc.sql) and
run it too. This adds the reliable team-creation function (and repairs the team
INSERT rule). It is safe to re-run. Without it, creating a team fails with
*"new row violates row-level security policy for table teams"*.

Finally, run [`supabase/0003_fix_panel_crypto.sql`](../supabase/0003_fix_panel_crypto.sql)
in another **new query**. It points the panel encryption functions at the
`extensions` schema where Supabase keeps pgcrypto. Without it, saving a panel
fails with *"function pgp_sym_encrypt(text, text) does not exist"*. Also safe to
re-run.

> If you set up a **fresh** project from scratch, `0001` already contains this
> fix — you'd only need `0001` and `0002`. `0003` exists to repair projects
> that ran an earlier `0001`.

Then run [`supabase/0004_team_members.sql`](../supabase/0004_team_members.sql)
in a **new query**. It adds the functions for inviting and removing team
members by email. Also idempotent.

Then run [`supabase/0005_deploys.sql`](../supabase/0005_deploys.sql) in a
**new query**. It adds the deploy-history table and the `record_deploy`
function, so each project's Deploys tab can show who deployed what and when.
Also idempotent.

Then run [`supabase/0006_issues.sql`](../supabase/0006_issues.sql) in a
**new query**. It adds the issues and comments tables (and the `create_issue`
/ `add_issue_comment` functions) that power each project's Issues tab. Also
idempotent.

Then run [`supabase/0007_project_deletions.sql`](../supabase/0007_project_deletions.sql)
in a **new query**. It adds the tombstone table and `request_project_deletion`
function used by "delete everywhere", so a deleted project is also removed from
every teammate's machine. Also idempotent.

Then run [`supabase/0008_profiles.sql`](../supabase/0008_profiles.sql) in a
**new query**. It adds the profile fields (location, website, logo/avatar and a
Markdown README) to user accounts and teams, restricts team editing to the
owner, and adds `set_member_role` so the owner can grant or revoke admin
rights. Also idempotent.

Then run [`supabase/0009_commits.sql`](../supabase/0009_commits.sql) in a
**new query**. It adds the cloud-commit and deploy-bundle tables (metadata
only — the file snapshots live on the storage backend) and their RPCs, which
power the reworked Deploy/commit/history flow. Also idempotent.

Then run [`supabase/0010_commit_manifests.sql`](../supabase/0010_commit_manifests.sql)
in a **new query**. It adds per-commit and per-deploy content manifests (so a
"local vs server" diff needs no download), the `finalize_commit` /
`server_manifest` functions, and the manifest-aware `release_bundle`. Also
idempotent.

Then run [`supabase/0011_issue_links.sql`](../supabase/0011_issue_links.sql)
in a **new query**. It links issues to deploys and commits: a new issue is
filed against the current Deploy, and a resolved issue can be pinned to the
commit that fixed it (`assign_issue_commit`). Also idempotent.

Then run [`supabase/0012_project_logo.sql`](../supabase/0012_project_logo.sql)
in a **new query**. It adds a logo image URL to projects. Also idempotent.

Then run [`supabase/0013_server_baseline.sql`](../supabase/0013_server_baseline.sql)
in a **new query**. It stores a project-level **server-state baseline**
(`set_server_manifest`, and a `server_manifest` / `release_bundle` that read and
write it), so the "changes since last deploy" diff is correct immediately after
you import a server's files instead of showing every file as changed. Also
idempotent.

> **v2.5 needed no new migration.** The delta-commit / bundle-deploy rework in
> v2.5 changed only the *storage* format (a commit zip is now a delta, and each
> deploy stores a full rollback snapshot) — the database schema (`0001`–`0013`)
> was unchanged. If you're upgrading a pre-2.5 test setup, start the storage
> area fresh: older full-snapshot commits aren't compatible with delta deploys.
> The database is unaffected.

Then run [`supabase/0014_image_storage.sql`](../supabase/0014_image_storage.sql)
in a **new query**. It creates a public **`images`** Storage bucket (with
read-for-all / write-for-authenticated policies) so avatars and logos can be
**uploaded from a file** instead of pasted as a URL. Also idempotent.

Then run [`supabase/0015_invite_by_username.sql`](../supabase/0015_invite_by_username.sql)
in a **new query**. It recreates `invite_member` so a teammate can be added by
their **email address or their Feather username**. Also idempotent.

Then run [`supabase/0016_commit_details.sql`](../supabase/0016_commit_details.sql)
in a **new query**. It adds an optional **`description`** to commits and a
`delete_commit` function that removes the **newest** commit of a Deploy that
hasn't shipped yet (LIFO). Also idempotent.

Then run [`supabase/0017_public_read.sql`](../supabase/0017_public_read.sql)
in a **new query**. It opens up **reads**: any signed-in user can view teams,
who's on them, projects, deploy history, commits and issues, so profile and team
pages show the full picture (all of a person's teams/projects, not just the ones
you share) and you can browse another team's project read-only. **Writes are
unchanged**, and **panels stay members-only** (they hold the encrypted API
keys). Also idempotent.

Last, run [`supabase/0018_deploy_details.sql`](../supabase/0018_deploy_details.sql)
in a **new query**. It adds an optional **`description`** to a Deploy (alongside
its name) and recreates `release_bundle` to accept it. Also idempotent.

> **v2.6 adds five migrations.** Run `0014`, `0015`, `0016`, `0017` and `0018`
> (above) once, in order, after `0001`–`0013`. `0014` also creates the `images`
> Storage bucket — avatars and logos won't upload until it exists.

## 3b. Deploy the storage function (cloud commits)

Feather stores commit snapshots and rollbacks as files on a dedicated
Pterodactyl server, reached only through the **`feather-storage`** Edge
Function so its API key never ships in the app. Follow
[`supabase/functions/feather-storage/README.md`](../supabase/functions/feather-storage/README.md):
set the `FEATHER_STORAGE_KEY` secret and run `supabase functions deploy
feather-storage`. Until it's deployed, Feather treats cloud storage as
unavailable — so this step is safe to do whenever you're ready.

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
