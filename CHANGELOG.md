# Changelog

All notable changes to Feather are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Issues connected to deploys and commits (M23).** A new issue is
  automatically filed against the project's **current Deploy**, so a deploy's
  page in History lists the issues raised in that cycle (open and fixed). On
  an issue you can pin the **commit that fixed it** — a dropdown of that
  Deploy's commits — and that commit's detail page then shows the issues it
  resolved. Requires migration `0011`.
- **History with Deploys & Commits, diffs, and storage rollback (M22).** The
  project's **History** now has two categories — **Deploys** (every released
  bundle) and **Commits** (every commit) — each opening a detail view. A
  commit's page shows its full file diff against the previous commit; a
  deploy's page lists the commits it shipped and its diff against the previous
  deploy. From a commit you can **Rollback**: Feather downloads that commit's
  snapshot from the storage backend and redeploys it (the local folder is
  never touched), so any teammate can roll the server back to any commit even
  without those files locally.
- **Cloud commits in the Deploy tab (M22).** The Deploy tab now opens with a
  **"Changes since last deploy"** panel: a live diff of your local folder
  against the current server state (added/modified/deleted, with an
  expandable file list). Commit your changes with a message (e.g.
  "Commit v2.4.0") and the snapshot is packed and uploaded to the storage
  backend and added to the project's **current Deploy** — a shared bundle
  every teammate's commits accumulate into, shown with who committed what.
  Pressing **Deploy** ships the files (unchanged, proven engine) and releases
  the bundle, recording the deployed state so the diff resets and a fresh
  Deploy opens. Requires migrations `0009` + `0010` and the `feather-storage`
  function. The old per-project git commit box is replaced by this flow.
- **Cloud-commit backend scaffolding (M22, foundation).** Groundwork for the
  reworked Deploy/commit/history flow where each member commits locally and
  everyone's commits bundle into the project's current Deploy: a new
  `supabase/0009_commits.sql` migration (metadata-only `commits` and
  `deploy_bundles` tables with `current_bundle` / `create_commit` /
  `mark_commit_stored` / `release_bundle` RPCs), and the **`feather-storage`
  Edge Function** — the single server-side holder of the storage server's API
  key, which authenticates each caller, checks team membership, derives the
  storage path itself and self-creates the folder tree. The commit/deploy UI
  wiring lands in following steps; deploying the function early is harmless.

### Security

- **Reserved storage backend is fully excluded.** Feather is gaining a
  behind-the-scenes Pterodactyl server that holds commit snapshots and
  rollbacks (M22). That server is now hard-excluded from every ordinary code
  path: it is filtered out of server listings and rejected from every
  server-scoped operation (details, resources, power, console, files,
  backups, deploy), so even a user who connects a panel at the same host with
  a key that can see it can never list, import, browse or deploy to it. Its
  API key lives only server-side (never in the app).

### Added

- **Profile pages for users and teams.** Every account and every team now has
  a self-customizable, GitHub-style profile — a logo/avatar, location, website
  and a Markdown README — reachable from the account menu ("Your profile",
  "Team profile") and by clicking a teammate in the Members list. You edit your
  own account profile; a **team's page is editable only by its owner**. Team
  creation gained matching optional fields (location, website, logo, README).
  Requires the new `supabase/0008_profiles.sql` migration.
- **Owner-granted admin rights.** The team owner can promote a member to admin
  ("Make admin") or demote them ("Remove admin") from the Members tab, backed
  by an owner-only `set_member_role` function; direct role changes and team
  edits are locked to the owner at the database level.
- **A GitHub-style project Overview.** The Overview now opens with a stat row
  — open issues, total deploys, the last deploy (with a status dot and
  relative time) and the commit currently on the server — each tile jumping
  to the matching tab. Below the About/README, a **Recent activity** card
  lists the newest deploys and rollbacks (status, commit, who and when) with
  a shortcut to the full history, so a project's state and momentum are
  visible at a glance.
- **One click from a project to its server.** A project page now links
  straight to its imported server's tile in the **Panels** tab — an "Open in
  Panels ↗" button in the project header and the clickable server id in the
  Overview sidebar. The Panels tab switches, scrolls the tile into view and
  gives it a brief highlight ring (respecting reduced-motion).

### Changed

- Completed the rename from the project's old codename to **Feather**: the
  Rust core crate is now `feather-core` (was `wingman-core`), and the HTTP
  user-agent, gradient id and internal comments no longer reference the old
  name. Deploy backups were already named `feather-pre-deploy-*`.

### Fixed

- **Pre-deploy backups now surface when they can't be taken.** The engine
  already polls each `feather-pre-deploy-*` backup to completion and only
  proceeds once the panel reports success, but a skipped backup (the server
  has no backup slots, or every slot is occupied by a backup Feather didn't
  create and so won't rotate) was silently swallowed by the UI. The Deploy
  tab now shows a persistent amber warning for the rest of the run, and the
  desktop sends a "No backup taken" notification, so "Back up the server
  before each deploy" can never fail quietly.

## [2.2.0] — 2026-07-22

Reworked the app around a clearer split: **Panels** for live server operation,
**Projects** for planning and deploying. See the [README](README.md) for the
full feature guide.

### Added

- **Two ways to delete a project.** *Remove from Feather* deletes the project
  (and its issues and history) for the team but keeps everyone's local files.
  *Delete everywhere* also removes the linked local folder on every teammate's
  machine — a tombstone (`supabase/0007`) is recorded and each Feather acts on
  it at launch. A safety guard refuses to delete shallow paths.

### Changed

- **Panels tab now shows every server across all the team's panels** at once,
  each grouped under its panel, with power, live stats and console. The Rust
  core supports several panels connected simultaneously (server commands are
  scoped by panel). Deploy, git history and the file browser are moving off the
  server tiles and into the matching project (Projects tab).
- **Deploy, history and files now live in the project.** Each project page
  gains a **Deploy** tab (deploy button with live progress, "Import from
  server", commit local changes, git history with one-click rollback, and the
  shared deploy timeline) and a **Files** tab (the server file browser),
  wired to the project's imported server and this device's local folder. The
  deploy engine now takes the full project config from the app (built from the
  cloud project + local folder), so there's no separate local project store.
- **Projects now import a server.** Creating a project means picking a panel
  (required) and one of its existing servers — its RAM/CPU/disk limits are
  shown, and servers already imported are marked. A local folder is optional:
  add one to deploy from this device, or leave it out to plan and manage
  across servers without keeping files locally. The chosen folder is a
  per-device binding, so each teammate can pick their own (or none). Server
  creation/deletion isn't offered — Pterodactyl's client API can't do it.
- The Linux app icon now resolves correctly: the bundle identifier was aligned
  with "Feather" so the desktop matches the window to its launcher/taskbar icon.

## [2.1.0] — 2026-07-22

Cloud collaboration — Feather grows from a local-only app into a team
platform backed by a free Supabase project. The deploy engine still runs
locally on each machine; the cloud holds the shared data. See the
[README](README.md) for the full feature guide and
[docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md) to set up the backend.

### Added

- **Accounts & teams** — sign in with an email account and work inside a
  team. A team is the unit of collaboration: its panels, projects, history
  and issues are shared by everyone on it. Row-Level Security means you only
  ever see the teams you belong to.
- **Shared, encrypted panels** — Pterodactyl connections now live in the
  team, not on one machine. Multiple panels are supported, and every
  teammate can connect and deploy without re-entering keys. API keys are
  encrypted at rest with a key kept in Supabase Vault and are only ever
  decrypted for team members. On this device the decrypted key is held in
  memory for the session only — never written to local disk.
- New **Panels** screen to add, connect to and remove team panels.
- **Projects** — a shared, team-wide list of everything you're working on.
  Each project has a name and a description for plans and notes, can be linked
  to a team panel, and opens into its own detail view. Create, edit and delete
  are synced for the whole team. A **Projects / Panels** tab bar switches
  between planning and the server dashboard. (Deploy history, issues and
  richer planning attach to these projects in later milestones.)
- **Team members** — a Members tab to see who's on the team and, for
  admins/owners, add teammates by email or remove them. The team owner is
  always protected. New teammates immediately share the team's projects,
  panels and history.
- **Project detail page** — clicking a project opens a GitHub-style page with
  an **Overview** (an editable "About" description plus a sidebar showing the
  linked panel, server, deploy target, post-deploy behaviour, and who created
  it when) and a **Settings** tab to edit every field, with a delete
  danger-zone. The tab bar is where Issues and Planning will live.
- **Deploy history** — a **Deploys** tab on each project shows every deploy and
  rollback the team has run: status, commit, file count, who ran it and when.
  Deploys and rollbacks from the server dashboard are recorded automatically
  against the matching project (created on first deploy if needed), so the
  whole team sees a shared timeline.
- **Issues** — a GitHub-style issue tracker on each project. Open issues
  (numbered per project), write a description, discuss in comments, and close
  or reopen them. Open/closed filters and comment counts included; everything
  is shared across the team.
- **Markdown planning** — project descriptions, issues and comments render
  Markdown: headings, bold/italic, lists, code, blockquotes, links and
  GitHub-style task lists. Checklists in a project's description are
  interactive — tick `- [ ]` items right on the Overview and the change is
  saved for the team. The renderer is dependency-free and escapes all input,
  so no untrusted HTML or `javascript:` links can run in the app.

### Changed

- The single local panel config (OS keychain / `credentials.json`) is
  replaced by the shared cloud panels above; the local keychain dependency
  was removed.

### Fixed

- Team creation now goes through a `SECURITY DEFINER` function
  (`supabase/0002_team_create_rpc.sql`), fixing a row-level-security error
  that could block creating a team.
- Panel key encryption/decryption now resolves `pgcrypto` in the
  `extensions` schema (`supabase/0003_fix_panel_crypto.sql`), fixing a
  "function pgp_sym_encrypt does not exist" error when saving a panel.

## [2.0.0] — 2026-07-19

### Changed

- Version set to 2.0.0.

## [1.2.1] — 2026-07-19

### Changed

- The GitHub repository was renamed to `Timmybott/Feather`; the updater
  endpoint, installer and all documentation links now point there.

## [1.2.0] — 2026-07-19

### Added

- **Initial import** — linking a project to a server with an empty local
  folder now downloads the server's current files into it (and creates the
  first git checkpoint automatically).
- **Multi-device sync** — every deploy leaves a small state marker
  (`.feather-state.json`) on the server. Other devices poll it and, when a
  newer deploy exists and their working tree is clean, automatically pull
  the server state into their local folder. Local uncommitted changes are
  never overwritten — you get a console note instead.
- **Update popup** — when a new release is available, a dialog appears
  right at startup with one-click "Install & restart".
- **Real logo** — a wing mark replaces the placeholder "W", identical in
  the header and the app icons.

### Changed

- **Renamed to Feather** — the app is now called Feather (same logo).
- Panel responses with `null` resource limits (seen on real panels) are
  handled everywhere.
- API keys: if the OS keychain is unavailable (e.g. Linux without a Secret
  Service), the key now falls back to an obfuscated file in the config
  directory instead of failing — see the README's security section.

### Fixed

- Backup limit handling when the panel reports no backup limit.

## [0.5.0] — 2026-07-19

First feature-complete version — everything from the v1 specification.

### Added

- **Panel connection & dashboard** — connect a Pterodactyl panel with a
  client API key (stored in the OS keychain, never in plain text) and see
  all servers as tiles with live status, CPU and RAM.
- **Live servers** — power actions (start/stop/restart, two-step kill),
  streamed console with command input, live stats over the Wings websocket
  with automatic token refresh and reconnect.
- **One-click deploys** — link a local project folder to a server and ship
  it: zip → upload via the panel's signed URL → extract, with
  `.deployignore` (gitignore syntax), configurable target directory,
  post-deploy restart or desktop notification, and live progress on the
  server tile. Files deleted locally are removed remotely via manifest
  diff.
- **Versioning** — every project is a real git repository (auto-initialized
  and auto-committed on deploy), commit UI and history with one-click
  rollback that never touches the working tree, automatic pre-deploy
  backups with rotation (only Feather's own backups are ever rotated), and
  an optional build command with streamed output.
- **File browser** — navigate server files, create folders, delete files
  and folders.
- **Auto-updater** — the app checks GitHub releases and updates itself with
  one click.
- **Easy install** — Windows NSIS installer and a one-line Linux installer
  (`install.sh`, .deb on apt-based distros, AppImage elsewhere).

[2.0.0]: https://github.com/Timmybott/Feather/releases/tag/v2.0.0
[1.2.1]: https://github.com/Timmybott/Feather/releases/tag/v1.2.1
[1.2.0]: https://github.com/Timmybott/Feather/releases/tag/v1.2.0
[0.5.0]: https://github.com/Timmybott/Feather/releases/tag/v0.5.0
