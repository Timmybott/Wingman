# Changelog

All notable changes to Feather are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Deploy-history rows open their deploy.** Each row in a project's Deploy-tab
  **Deploy history** is now clickable — it opens the shared history focused on
  that deploy, where you can see its commits and click any changed file for a
  line-level diff.
- **Profiles & team pages cross-link.** A user's profile now lists the **teams**
  and **projects** they're part of, each clickable. A team page lists all its
  **members** (click through to their profile) and the team's **projects**.
  You can hop profile → team → member → project without leaving the app.
- **Panels tab: disk usage & project shortcuts.** Each server tile now shows a
  **Disk** meter alongside CPU and RAM. Servers that have a Feather project are
  marked with a project chip you can click to **jump straight to that project**.
- **Edit server files in place.** In a project's **Files** tab, click a file to
  open it in an editor and save changes straight back to the server — work
  directly on the server without a local copy. Text files up to ~1 MB;
  non-text/oversized files open read-only.
- **Clickable per-file line diffs.** In the Deploy tab's "Changes since last
  deploy" and in History (a commit's detail), click any changed file to see a
  GitHub-style line-level diff — added, removed and changed lines. Deploy-tab
  diffs compare the server's version to your local file; commit diffs compare
  the commit's snapshot to its parent's.
- **Project logos.** A project can carry a logo image (URL), shown on its page
  and in the project list. Set it under Settings. Requires migration `0012`.

### Changed

- **Overview reworked.** The **Local folder** binding moved from the Overview
  to **Settings**, and linking an empty folder now **imports the server's
  files automatically**. The Overview sidebar shows the **team** (click to open
  the team page) and the creator is clickable to their profile.

### Fixed

- **Diff no longer shows every file as changed after linking a folder.** The
  Deploy tab diffed against the last *released* bundle, so a freshly imported
  project (no cloud deploy yet) treated every file as new. The server-state
  baseline now lives on the project and is set both when the server's files
  are imported and when a deploy is released (migration `0013`).
- **Overview stats & recent activity now stay current.** The stat tiles (open
  issues, deploys, last deploy, current commit) and the Recent-activity list
  loaded only once and went stale — showing 0 / "No deploys yet" after a
  deploy. They now reload every time the Overview is opened.
- **Closed issues can be linked to the commit that fixed them.** The "Fixed in"
  picker on an issue previously appeared only for issues filed against a Deploy
  and listed only that one Deploy's commits — so an issue whose fix landed in a
  later cycle (the usual case once it's closed) could never be linked. It now
  lists **every commit in the project**, grouped by deploy, for any issue.

## [2.3.0] — 2026-07-22

A team-collaboration release. Feather's deploy/commit/history flow is reworked
around **cloud commits**: you work locally, see a live diff against the server,
commit snapshots into a shared **Deploy** that everyone's work bundles into, and
ship it in one click — with a full **Deploys/Commits history**, diffs and
**rollback** to any past commit. Accounts and teams get **profile pages**, and
issues now connect to the deploys and commits that address them. Snapshots are
stored on a dedicated backend reached only through a key-holding Edge Function.
Requires migrations `supabase/0008`–`0011` and the `feather-storage` function
(see [docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md)).

### Added

- **Cloud commits & the reworked Deploy tab.** The Deploy tab opens with a
  **"Changes since last deploy"** panel — a live diff of your local folder
  against the current server state (added/modified/deleted, with an expandable
  file list). Commit with a message (e.g. "Commit v2.4.0") and the snapshot is
  packed and uploaded to the storage backend, joining the project's **current
  Deploy** — a shared bundle every teammate's commits accumulate into, shown
  with who committed what. **Deploy** ships the files (via the proven deploy
  engine) and releases the bundle, recording the deployed state so the diff
  resets and a fresh Deploy opens. Replaces the old per-project git commit box.
- **History with Deploys & Commits, diffs and rollback.** A project's
  **History** now has two categories — **Deploys** (every released bundle) and
  **Commits** (every commit) — each with a detail page. A commit's page shows
  its full diff against the previous commit; a deploy's page lists the commits
  it shipped and its diff against the previous deploy. **Rollback** downloads a
  past commit's snapshot from the storage backend and redeploys it (the local
  folder is never touched), so anyone can roll the server back to any commit
  even without those files locally.
- **Issues connected to deploys and commits.** A new issue is filed against the
  project's current Deploy, so a deploy's page lists the issues raised in that
  cycle (open and fixed). On an issue you can pin the **commit that fixed it**
  from a dropdown of that Deploy's commits; the commit's detail page then shows
  the issues it resolved.
- **Profile pages for users and teams.** Every account and team has a
  self-customizable, GitHub-style profile — logo/avatar, location, website and a
  Markdown README — reached from the account menu ("Your profile", "Team
  profile") or by clicking a teammate in the Members list. You edit your own
  account profile; a **team's page is editable only by its owner**. Team
  creation gained matching optional fields.
- **Owner-granted admin rights.** The team owner can promote a member to admin
  or demote them from the Members tab, backed by an owner-only `set_member_role`
  function; direct role changes and team-profile edits are locked to the owner
  at the database level.
- **A GitHub-style project Overview.** The Overview opens with a stat row — open
  issues, total deploys, the last deploy (status dot + relative time) and the
  commit currently on the server — each tile jumping to the matching tab. Below
  the About/README, a **Recent activity** card lists the newest deploys and
  rollbacks.
- **One click from a project to its server.** A project links straight to its
  imported server's tile in the **Panels** tab — an "Open in Panels ↗" button
  and a clickable server id — which switches, scrolls the tile into view and
  highlights it (respecting reduced-motion).

### Changed

- Completed the rename from the project's old codename to **Feather**: the Rust
  core crate is now `feather-core` (was `wingman-core`), and the HTTP
  user-agent, gradient id and internal comments no longer reference the old
  name. Deploy backups were already named `feather-pre-deploy-*`.

### Fixed

- **Pre-deploy backups now surface when they can't be taken.** The engine polls
  each `feather-pre-deploy-*` backup to completion and only proceeds once the
  panel reports success, but a *skipped* backup (no backup slots, or every slot
  held by a foreign backup Feather won't rotate) was silently swallowed by the
  UI. The Deploy tab now shows a persistent amber warning and the desktop sends
  a "No backup taken" notification, so "Back up the server before each deploy"
  can never fail quietly.

### Security

- **A reserved storage backend, fully excluded from normal use.** Snapshots and
  rollbacks live on a dedicated Pterodactyl server whose API key never ships in
  the app — it lives only in the **`feather-storage`** Supabase Edge Function,
  which authenticates each caller, checks team membership and derives every
  storage path server-side (self-creating the folder tree). That server is
  hard-excluded from every ordinary Feather code path — filtered from listings
  and rejected from all server-scoped operations — so even a user who connects a
  panel at the same host with a key that can see it can never list, import,
  browse or deploy to it.

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

[2.3.0]: https://github.com/Timmybott/Feather/releases/tag/v2.3.0
[2.2.0]: https://github.com/Timmybott/Feather/releases/tag/v2.2.0
[2.1.0]: https://github.com/Timmybott/Feather/releases/tag/v2.1.0
[2.0.0]: https://github.com/Timmybott/Feather/releases/tag/v2.0.0
[1.2.1]: https://github.com/Timmybott/Feather/releases/tag/v1.2.1
[1.2.0]: https://github.com/Timmybott/Feather/releases/tag/v1.2.0
[0.5.0]: https://github.com/Timmybott/Feather/releases/tag/v0.5.0
