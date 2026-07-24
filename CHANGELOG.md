# Changelog

All notable changes to Feather are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [2.6.3] — 2026-07-23

Better commit/deploy forms and genuinely automatic team sync.

> **New database migration.** Run `supabase/0018_deploy_details.sql` once in the
> Supabase SQL editor (after 0001–0017). See
> [docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md).

### Added

- **A deploy has a name and description too.** Like a commit, a deploy now takes
  an optional name and a Markdown description (shown in its history entry) —
  migration `0018`.

### Changed

- **Commit and deploy fields are stacked.** The name field sits on its own line
  with the taller description below it and the button underneath, instead of the
  fields sitting side by side.
- **Deploys sync to the whole team automatically.** When a teammate ships a
  deploy, every other member's local folder now updates on its own — while the
  app is open (you no longer need the project's Deploy tab in front of you), and
  on the next launch for anyone who was offline.
- **Auto-sync no longer stalls on the normal case.** A new deploy is, of course,
  different from what teammates have locally; that alone used to block the sync
  behind a "commit or discard" banner. Feather now compares file contents and
  pulls the deploy whenever it's safe, holding back **only** when the pull would
  overwrite an **un-deployed local edit to a file the deploy doesn't change** —
  so your uncommitted work is protected without getting in the way of routine
  updates.

## [2.6.2] — 2026-07-23

Profile and team pages now show the **whole** picture, not just what you share.

> **New database migration.** Run `supabase/0017_public_read.sql` once in the
> Supabase SQL editor (after 0001–0016). See
> [docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md).

### Fixed

- **Profiles only showed teams and projects you shared with the person.** If you
  and someone were both on team *abc* but they were also on team *xyz*, their
  profile listed only *abc*. A profile now lists **all** of a person's teams and
  projects (and a team page shows its real members and projects), because
  signed-in users can now read teams, projects and their content — Feather is
  GitHub-like and its projects are open source (migration `0017`).

### Changed

- **Reads are open to any signed-in user; writes are unchanged.** Teams, who's on
  them, projects, deploy history, commits and issues are readable by anyone
  signed in, so you can browse another team's work read-only. Creating or
  changing anything still requires the right membership/role, **panels stay
  members-only** (they hold the encrypted API keys), and commit/deploy file
  bytes still download only through the membership-checked storage function.
- **Opening issues and commenting** on another team's project stay available when
  you're a **member** of that team; a project you reach purely as an outsider
  (via a public profile) is **view-only** for issues, so there's no button that
  would just fail.

## [2.6.1] — 2026-07-23

A fix-and-feature release: you can now open another team's projects.

### Fixed

- **Viewing another team's project no longer dead-ends.** If you belong to two
  teams and opened a project of the team you weren't currently in (from your
  user page), Feather showed *"This project is no longer available."* It now
  loads that project — read-only.

### Added

- **Read-only cross-team project view.** Because all Feather projects are open
  source, you can open any project of a team you're on, even one that isn't your
  active team, and:
  - browse its **Overview**, **Files** (view only) and full **History** (past
    deploys and commits with diffs);
  - **open issues** and **comment** on them, to report bugs from the outside.

  What stays off-limits for another team's project: **Settings**, **Deploy**,
  **Commit**, **Import from server**, **Rollback**, editing or deleting files,
  and **closing/reopening** issues or pinning a fixing commit. Such a project is
  marked **Read-only**, and its panel is connected on demand so its files and
  server state are reachable.

## [2.6.0] — 2026-07-23

A workflow and polish release: richer commits, full-page views with a real
Back button, file-based image uploads, a guided team-creation flow, and
statistics across team and user pages.

> **New database migrations.** Run `supabase/0014_image_storage.sql`,
> `supabase/0015_invite_by_username.sql` and `supabase/0016_commit_details.sql`
> once in the Supabase SQL editor (in order, after 0001–0013). `0014` also
> creates a public **`images`** storage bucket for avatars and logos. See
> [docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md).

### Added

- **Add members by email _or_ username.** The "Add member" field accepts either
  a teammate's email address or their Feather username (migration `0015`).
- **Commit name and description.** A commit now has a name and an optional
  Markdown description, written with the rich-text toolbar (migration `0016`).
- **View a commit's file diffs in the current Deploy.** Each commit in the
  current Deploy expands to show its per-file changes; click a file for the
  line-level diff.
- **Remove a commit from the current Deploy.** The newest commit of a pending
  Deploy can be removed (LIFO — later commits build on earlier ones).
- **Upload images from a file.** Avatars and logos (user, team, project) are now
  chosen from a file on your computer and uploaded to Supabase Storage, instead
  of pasting a URL (migration `0014`, `images` bucket).
- **Guided team creation.** Creating a team runs a short wizard — name, then
  logo, then an "about" README — instead of a single form.
- **Statistics on team and user pages.** The team page summarises its projects,
  members, open issues and total deploys; the user page summarises teams and
  projects. (The project page already carried its own stats.)
- **Rich-text toolbar on description fields.** Bold, italic, headings, lists,
  quotes, code and links, on every Markdown description/README editor.

### Changed

- **Full-page views instead of drawers and modals.** The server console, project
  history/rollback, the server file editor and file diffs are now full pages
  with their own back button, rather than slide-in drawers or pop-up modals.
- **A real Back button.** Navigation is backed by a stack, so Back always returns
  to the page you actually came from — a profile opened from inside a project
  returns to that project, not the projects list.
- **Prettier inputs and dropdowns.** Text fields, text areas and select menus
  share a consistent rounded style with a focus glow and a custom chevron.
- **Emoji removed** from the interface in favour of plain text and typographic
  symbols.

## [2.5.1] — 2026-07-23

A bug-fix release.

### Fixed

- **Snapshot downloads no longer fail with HTTP 400.** The storage function read
  files with Pterodactyl's `files/contents` endpoint, which is meant for text
  and rejects binary/large files (our zips) — so every snapshot download failed,
  breaking **deploys** and **commit file diffs**. It now fetches a signed
  download URL (`files/download`), the same reliable path the sync pull uses.
  **Redeploy the Edge Function** for this fix: `supabase functions deploy
  feather-storage`.
- **Opening a project's team no longer errors.** The team link passed the click
  event as the team id (`invalid input syntax for type uuid: "[object
  PointerEvent]"`); it now opens the project's team correctly.
- **Member avatars show up.** The Members list and a team page rendered only the
  initial letter — `listMembers` didn't fetch `avatar_url`. Members with an
  avatar now show their picture (initial as a fallback).

## [2.5.0] — 2026-07-23

A deploy-model release. Commits and deploys are reworked so a **deploy ships
exactly the committed work — nothing else**. A **commit** now records only its
*delta* (what changed since the last commit); a **deploy** applies the
accumulated deltas of the current bundle to the server and introduces no
changes of its own. This makes deploys the true sum of their commits: different
members' commits to different files combine cleanly, and uncommitted local
edits are never shipped. After a deploy, teammates' local folders **sync
automatically**, and **rollback** restores a past *deploy* from a full snapshot
taken at deploy time. No new database migration — the change is in the storage
format (commit zips are now deltas) and the engine.

> **Upgrading from a pre-2.5 setup:** commit zips changed from full snapshots to
> deltas, and rollback now targets deploys, so existing commit/deploy history
> from an earlier build isn't compatible. Start the storage area fresh (the
> database is unaffected).

### Changed

- **A commit stores only its delta.** Instead of a full snapshot of your folder,
  a commit records just the files it changed relative to the accumulated
  committed state, and uploads only those. The commit still knows the full
  resulting tree, so a deploy can apply the whole bundle.
- **A deploy applies its bundle's commits — and nothing else.** Pressing
  **Deploy** downloads the current bundle's commit deltas and applies them over
  the server's state; **uncommitted local edits are never deployed**, and a
  teammate without a local folder can deploy just the same. A deploy with no
  commits is blocked ("commit your changes first").
- **Deploys combine everyone's commits.** Because commits are deltas, two
  members who changed *different* files both land in the next deploy — the
  previous full-snapshot model could only ship one member's folder.
- **History reflects the model.** A deploy's detail lists exactly the commits it
  shipped (no separate "changes on the server" — a deploy changes nothing on its
  own); open a commit for its own line-level diff.
- **Rollback restores a deploy, not a single commit.** Each deploy stores a
  complete snapshot of the deployed tree; **Rollback to this deploy** (on a
  deploy's detail) restores it in full, without touching your local folder.

### Added

- **Teammates' deploys sync automatically.** The Deploy tab watches the server's
  deploy marker (on open and every 30 s) and pulls a newer deploy into your
  local folder when your working tree is clean — so everyone stays on the latest
  state. A dirty tree is never overwritten; a banner asks you to commit or
  discard first, then it syncs on the next check.

### Fixed

- **The diff is correct again right after a rollback.** Rolling back now resets
  the project's server-state baseline to the restored deploy, so "changes since
  last deploy" no longer measures against the wrong state (the long-standing
  edge noted in the spec).

## [2.4.0] — 2026-07-22

A project-experience release. Everything inside a project is clearer and more
clickable: the **Overview** shows the team and creator as links, the **Deploy
tab** separates uncommitted local edits from what will ship and lets you drill
into any past deploy, **profiles and team pages** cross-link teams, projects and
members, the **Files tab** edits files straight on the server, and the **Panels
tab** shows disk usage and jumps to a server's project. Diffs are now clickable
down to the line, projects can carry a logo, and several Overview/diff/issue
bugs are fixed. Requires migrations `supabase/0012`–`0013` (see
[docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md)).

### Added

- **Uncommitted changes are called out in the Deploy tab.** The cloud-commit
  panel now shows a separate **Uncommitted local changes** block — the edits
  you've made since your last commit (distinct from the total "changes since
  last deploy"). Click any file for a line-level diff of the last commit's
  snapshot against your working copy, so you always know what still needs
  committing before the next deploy.
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

[2.6.3]: https://github.com/Timmybott/Feather/releases/tag/v2.6.3
[2.6.2]: https://github.com/Timmybott/Feather/releases/tag/v2.6.2
[2.6.1]: https://github.com/Timmybott/Feather/releases/tag/v2.6.1
[2.6.0]: https://github.com/Timmybott/Feather/releases/tag/v2.6.0
[2.3.0]: https://github.com/Timmybott/Feather/releases/tag/v2.3.0
[2.2.0]: https://github.com/Timmybott/Feather/releases/tag/v2.2.0
[2.1.0]: https://github.com/Timmybott/Feather/releases/tag/v2.1.0
[2.0.0]: https://github.com/Timmybott/Feather/releases/tag/v2.0.0
[1.2.1]: https://github.com/Timmybott/Feather/releases/tag/v1.2.1
[1.2.0]: https://github.com/Timmybott/Feather/releases/tag/v1.2.0
[0.5.0]: https://github.com/Timmybott/Feather/releases/tag/v0.5.0
