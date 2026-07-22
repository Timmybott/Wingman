# Feather

**A cloud-backed, team-collaborative desktop client for [Pterodactyl](https://pterodactyl.io) — think GitHub Desktop crossed with GitHub itself, but for your game servers, bots and self-hosted services.**

Feather splits cleanly in two: **Panels** is where you run your servers (power, live stats, console across every panel your team uses), and **Projects** is where you plan, track and deploy them. Each project imports a server and gets a GitHub-style page — a Markdown README, an issue tracker, and a **cloud-commit deploy flow**: you work in a local folder, see a live diff against the server, commit snapshots into a shared **Deploy** that everyone's work bundles into, ship it in one click, and browse a full **Deploys/Commits history** with diffs and one-click **rollback** to any past commit. Everything is shared through a free cloud backend, so a whole team works from the same picture. The name plays on Pterodactyl's flight theme — its daemon is called [Wings](https://github.com/pterodactyl/wings).

> **Status:** v2.3 — the deploy/commit/history flow is reworked around **cloud commits**: local diff → commit → shared Deploy → ship → Deploys/Commits history with diffs and rollback. Adds **user & team profile pages**, **owner-granted admin rights**, and **issues linked to the deploys and commits** that address them. Commit snapshots live on a dedicated storage backend reached only through a key-holding Edge Function. On top of the earlier Panels (all servers of all team panels) and Projects (import a server, plan it, deploy it) foundation.

---

## Contents

- [What Feather does](#what-feather-does)
- [How it fits together](#how-it-fits-together)
- [Installation](#installation)
- [First-time setup](#first-time-setup)
- [Feature reference](#feature-reference)
  - [Accounts & sign-in](#accounts--sign-in)
  - [Teams](#teams)
  - [Members & admin rights](#members--admin-rights)
  - [Profiles](#profiles)
  - [Panels (encrypted & shared)](#panels-encrypted--shared)
  - [The Panels tab — your servers](#the-panels-tab--your-servers)
  - [Projects — import a server](#projects--import-a-server)
  - [The project page](#the-project-page)
  - [Local folders (per device)](#local-folders-per-device)
  - [Cloud commits & the current Deploy](#cloud-commits--the-current-deploy)
  - [Deploying](#deploying)
  - [History: Deploys & Commits](#history-deploys--commits)
  - [Rollback](#rollback)
  - [Files](#files)
  - [Issues (linked to deploys & commits)](#issues-linked-to-deploys--commits)
  - [Markdown planning & checklists](#markdown-planning--checklists)
  - [Deleting a project](#deleting-a-project)
  - [Multi-device sync](#multi-device-sync)
  - [Auto-updates](#auto-updates)
- [The storage backend](#the-storage-backend)
- [Security & privacy](#security--privacy)
- [Development](#development)
- [License](#license)

---

## What Feather does

Feather is organised around two tabs:

**🖥️ Panels** — live server operation. Feather connects to **all** of your team's Pterodactyl panels at once and shows **every server**, grouped by panel, with power actions (start/stop/restart/kill), live CPU/RAM and a streamed console. This is the "run it" surface.

**📦 Projects** — planning and deploying. A project **imports one of your servers** and gives it a GitHub-style page: a Markdown README and checklists, an issue tracker, a **cloud-commit deploy workflow** (local diff → commit → shared Deploy → ship, with a Deploys/Commits history and rollback), and a server file browser. This is the "build, plan and ship it" surface.

Both live inside a **team**, backed by a free [Supabase](https://supabase.com) project that everyone on the team shares. Nobody re-enters API keys, and everyone sees the same projects, commits, deploys, issues and history.

At a glance:

| Area | What you get |
|---|---|
| **Accounts & teams** | Email sign-in; a team is the shared unit for everything below |
| **Members & roles** | Invite teammates by email; the **owner** grants/revokes **admin** rights; owner protected |
| **Profiles** | GitHub-style profile pages for every account and team (logo, location, website, Markdown README) |
| **Panels** | Several Pterodactyl connections per team, API keys **encrypted at rest**, all connected at once |
| **Panels tab** | Every server of every panel: power, live CPU/RAM, streamed console |
| **Projects** | Import a server → its own page for planning, issues, deploys and files |
| **Cloud commits** | Live diff local-vs-server; commit snapshots into a **shared Deploy** everyone's work bundles into |
| **Deploy** | One click ships the Deploy to the server and syncs everyone; `.deployignore`, target dir, restart/notify, pre-deploy backup |
| **History** | **Deploys** and **Commits** categories with full diffs and per-commit detail pages |
| **Rollback** | Redeploy any past commit's snapshot from the storage backend — without touching your local folder |
| **Issues** | Per-project tracker linked to the **Deploy** it was filed against and the **commit** that fixed it |
| **Planning** | Markdown READMEs, issues & comments; interactive `- [ ]` checklists |
| **Delete** | Remove from Feather (keeps local files) or delete everywhere (incl. local folders) |

> **On creating/deleting servers:** Feather uses Pterodactyl's **client API**, which cannot create or delete servers or change their RAM/CPU/disk — only your hosting provider can. So Feather **imports and manages existing servers**; it shows their limits read-only.

---

## How it fits together

```
   ┌──────────────────────────────┐        ┌────────────────────────────┐
   │   Your machine (Feather)      │        │   Supabase (shared cloud)   │
   │                               │  auth  │                            │
   │  Svelte UI ── in-memory ────▶ │◀──────▶│  accounts · teams · members │
   │     │      panel keys (RAM)   │  data  │  profiles · encrypted keys  │
   │     ▼                         │◀──────▶│  projects · issues          │
   │  Rust core                    │        │  commits · deploy bundles   │
   │  (multi-panel · git · deploy  │        │  deploy history · tombstones│
   │   · snapshots · diffs)        │        │  (Row-Level Security)       │
   │     │                         │        │                            │
   │     │                         │        │  feather-storage function ──┼──┐
   │     │  local folder bindings  │        │  (holds the storage key)    │  │
   └─────┼─────────────────────────┘        └────────────────────────────┘  │
         │ Pterodactyl client API (files, power, websocket) — per panel      │ commit
         ▼                                                                    │ snapshots
   ┌──────────────┐  ┌──────────────┐                       ┌─────────────────▼────────┐
   │  Panel A     │  │  Panel B     │  …                    │  Storage backend server  │
   └──────────────┘  └──────────────┘                       │  data/<team>/<project>/… │
                                                            └──────────────────────────┘
```

The **cloud never sees your plaintext API keys**, and Supabase stores only shared *metadata* — never your project files. Panel keys are decrypted for you (a team member) and handed to the Rust core **in memory only**. Which local folder a project deploys from is a **per-device binding** (each teammate picks their own, or none). Commit **snapshots** (the only file bytes Feather stores) go to a dedicated **storage backend** reached exclusively through the **`feather-storage`** Edge Function, which is the sole holder of that server's key — see [The storage backend](#the-storage-backend).

---

## Installation

> The first public release is in preparation — the download links go live with it. Until then, [build from source](#development).

**Windows:** download `Feather_x.y.z_x64-setup.exe` from the [latest release](https://github.com/Timmybott/Feather/releases/latest) and run it.

**Linux:** one line — installs the `.deb` on Debian/Ubuntu, the AppImage everywhere else:

```sh
curl -fsSL https://raw.githubusercontent.com/Timmybott/Feather/main/install.sh | bash
```

Once installed, Feather keeps itself up to date through its [built-in updater](#auto-updates).

---

## First-time setup

Feather needs a cloud backend for the collaboration features. It's a free Supabase project and takes a few minutes — **one person on the team does this once** and shares the two public values with everyone else.

### 1. Set up the cloud backend

Follow **[docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md)** step by step. In short you will:

1. Create a free Supabase project.
2. Create an encryption secret in Supabase Vault (used to encrypt panel keys).
3. Run the SQL migrations in [`supabase/`](supabase/) (`0001` … `0011`) in the SQL editor — they create every table, security policy and function.
4. Deploy the **`feather-storage`** Edge Function and set its `FEATHER_STORAGE_KEY` secret (the storage server's Pterodactyl key) — see [`supabase/functions/feather-storage/README.md`](supabase/functions/feather-storage/README.md). This powers cloud commits and rollback; until it's deployed Feather treats cloud storage as unavailable and deploys still work.
5. Turn on email login.
6. Copy your **Project URL** and **anon public key** into `src/lib/supabase.ts` (or hand them to whoever builds the app).

These two values are meant to live inside the app; your data is protected by database Row-Level Security, not by keeping them secret. **Never** share the service-role key or database password.

### 2. Create an account and sign in

Launch Feather. On first run you'll see the sign-in screen — create an account with your email and a password, then sign in. (If your Supabase project has "Confirm email" on, confirm via the email link first.)

### 3. Create or join a team

A **team** is where everything is shared. Create one (you become its owner) — optionally with a location, website, logo and README — or, if a teammate has already added you, pick an existing team. Switch teams any time from the header menu.

### 4. Add a panel

Open the **Panels** tab → **Manage panels → Add panel**. In your Pterodactyl account, create a client API key under **Account → API Credentials**, paste the panel URL and key, optionally **Test**, then **Add**. The key is encrypted in the cloud and shared with the team, and Feather connects to it automatically. Add as many panels as your team uses — a team needs at least one. Their servers now appear in the Panels tab.

### 5. Import a server as a project

Open the **Projects** tab → **New project**: pick a panel, pick one of its servers (its RAM/CPU/disk are shown), give it a name and description, and optionally **choose a local folder** to deploy from on this device. The project opens with its own Overview, Issues, Deploy and Files tabs.

### 6. Commit and deploy

On the project's **Deploy** tab, use **Import from server** to pull the server's current files into your local folder. Then edit locally: the **"Changes since last deploy"** panel shows a live diff. **Commit** your changes (they join the shared **current Deploy**), and press **Deploy** to ship the Deploy to the server. Everything lands in the project's **History** (Deploys & Commits).

---

## Feature reference

### Accounts & sign-in

Feather is account-based. Sign up with an email, display name and password; sign in on any machine to reach your teams. Sessions persist across restarts and refresh automatically. Log out from the header menu (top-right).

### Teams

A team is the unit of collaboration — its panels, projects, commits, deploys and issues are shared by everyone on it. You can belong to several teams and switch between them from the header (**Switch team**). Row-Level Security guarantees you only ever see teams you belong to. The team's creator is its **owner**. When you create a team you can add optional details — location, website, a logo and a Markdown README.

### Members & admin rights

The **Members** tab lists everyone on the team with their name, handle and role (**owner** / **admin** / **member**). Owners and admins can **add a member** by email (they must already have a Feather account) and **remove a member** (the owner is protected). Only the **owner** can grant admin rights — **Make admin** / **Remove admin** — and role changes and team-profile edits are locked to the owner at the database level. Clicking a teammate opens their profile. New members immediately share the team's panels, projects and history.

### Profiles

Every account and every team has a **profile page** — a GitHub-style overview with a logo/avatar, location, website and a Markdown **README**. Reach them from the account menu (**Your profile**, **Team profile**) or by clicking a teammate in the Members list. You edit your own account profile; a team's page can be edited **only by its owner**.

### Panels (encrypted & shared)

A panel is a connection to a Pterodactyl installation. Under **Panels → Manage panels** you add and remove panels; a team can have several and **needs at least one**.

- **Encrypted at rest.** API keys are encrypted with a master key kept in Supabase Vault and can only be decrypted by team members, through a database function that checks membership first. The raw key never leaves the database in plaintext.
- **Shared & auto-connected.** Feather connects to every team panel on launch, so all their servers are available without anyone re-entering keys.
- **In-memory on your device.** Decrypted keys are held in RAM for the session only and handed to the local core — never written to local disk.

### The Panels tab — your servers

The **Panels** tab aggregates the servers of **every** connected panel, grouped under each panel, and is purely about running them:

- **Live status, CPU and RAM**, streamed over the Wings websocket (token refresh + auto-reconnect).
- **Power actions** — start, stop, restart, and a two-click **kill**.
- **Console** — streamed live output with a command input.

Deploying, history and files are **not** here — they belong to the project. A project links straight to its imported server's tile here via **Open in Panels ↗** (the tab switches, scrolls to the tile and highlights it).

### Projects — import a server

The **Projects** tab is the team's home. **New project** imports one of a panel's servers:

1. Pick a **panel** (required).
2. Pick one of its **servers** — its RAM/CPU/disk limits are shown, and servers already imported are marked so you don't duplicate them.
3. Give it a name and description.
4. Optionally choose a **local folder** to deploy from on this device (add it now or later).

The result is a shared project everyone on the team sees. (You can also make a project purely for planning and cross-server management by leaving the local folder unset.)

### The project page

Clicking a project opens its GitHub-style page with a tab bar:

- **Overview** — a stat row (open issues, total deploys, last deploy, the commit currently on the server), the rendered [Markdown](#markdown-planning--checklists) description with interactive checklists, a **Recent activity** timeline, the [local-folder binding](#local-folders-per-device) for this device, and a metadata sidebar (linked panel, clickable server, deploy target, post-deploy behaviour, created by/when).
- **Issues** — the project's [issue tracker](#issues-linked-to-deploys--commits).
- **Deploy** — the [cloud-commit flow](#cloud-commits--the-current-deploy): the local-vs-server diff, commit box, the current Deploy, the **Deploy** button, **Import from server**, and **History**.
- **Files** — the server [file browser](#files).
- **Settings** — edit every field (name, description, panel, server, deploy target directory, build command, post-deploy behaviour, auto-backup) and [delete the project](#deleting-a-project).

### Local folders (per device)

Deploying pushes a **local folder** to the server, and that folder is chosen **per device** — each teammate binds their own copy (or none). The binding lives on your machine only; the cloud never learns your paths. Set, change or unlink it under **Overview → Local folder**, or when you create the project. Binding a folder makes it a git repository automatically.

### Cloud commits & the current Deploy

The Deploy tab opens with **"Changes since last deploy"** — a live diff of your local folder against the current server state (added `+`, modified `~`, deleted `−`, with an expandable file list). This is computed from lightweight content manifests, so no download is needed.

**Commit** your changes with a message (e.g. "Commit v2.4.0"). Feather packs your folder into a snapshot, uploads it to the [storage backend](#the-storage-backend), and adds the commit to the project's **current Deploy** — a shared bundle that *every teammate's* commits accumulate into. The current Deploy shows who committed what, so the whole team can see what's queued to ship.

### Deploying

Press **Deploy** to ship the current Deploy to the server. The proven deploy engine runs the pipeline:

1. **Commit** a git checkpoint of the folder.
2. **Build** — optionally run a per-project build command first, with streamed output.
3. **Back up** the server first (optional, on by default). If a backup can't be taken (no slots, or every slot holds a backup Feather didn't create and won't rotate), you get a persistent warning **and** a desktop notification — it never fails silently.
4. **Pack** the folder into a zip, honouring `.deployignore` (gitignore syntax).
5. **Upload & extract** through the panel's file API into the chosen **target directory** (server root by default, or a subfolder).
6. **Reconcile** — files you deleted locally are removed on the server via a manifest diff, so the server mirrors your folder.
7. **Release the Deploy** — the bundle is marked deployed (recording the server's new state so diffs reset), and a fresh Deploy opens for the next round.
8. **After deploy** — restart the server or just send a desktop notification, per project. Other teammates' devices [sync](#multi-device-sync) the new state.

Live progress (`Backing up… · Uploading 68%…`) shows on the tab. **Import from server** does the reverse — pulls the server's files into your folder — which is the safe way to start before your first commit.

### History: Deploys & Commits

The Deploy tab's **History** has two categories:

- **Deploys** — every released Deploy. A deploy's detail page lists the commits it shipped, its file diff against the previous deploy, and the [issues](#issues-linked-to-deploys--commits) raised in that cycle.
- **Commits** — every commit across the project. A commit's detail page shows its full file diff against the previous commit, the issues it fixed, and a **Rollback** button.

Everyone on the team sees the same history.

### Rollback

From any commit's detail page, **Rollback** re-deploys that commit's snapshot: Feather downloads the snapshot from the storage backend, extracts it, and runs the normal deploy pipeline from there — **your local folder is never touched**. Because snapshots live in the cloud, any teammate can roll the server back to any commit even without those files locally. Pre-deploy backups still run, so a rollback is itself recoverable.

### Files

The **Files** tab is a browser for the project's server: navigate directories, create folders and delete files, straight over the panel's file API. It works whether or not you have a local folder bound.

### Issues (linked to deploys & commits)

Each project has a GitHub-style **issue tracker**: open an issue with a title and Markdown description (numbered per project — #1, #2, …), discuss in comments, and close/reopen. Filter by **Open**/**Closed** and see comment counts at a glance.

Issues connect to the deploy flow:

- A **new issue is filed against the current Deploy**, so a deploy's History page lists the issues raised in that cycle (open and fixed).
- On an issue you can pin the **commit that fixed it** — a dropdown of that Deploy's commits — and the commit's detail page then shows the issues it resolved.

Everything is shared, and comments are attributed to their author.

### Markdown planning & checklists

Project descriptions, profile READMEs, issue bodies and comments render **Markdown** — headings, bold/italic, lists, inline code and code blocks, blockquotes, links, and GitHub-style **task lists**. Checklists in a project's description are **interactive**: tick `- [ ]` items right on the Overview and the change saves for the whole team, turning the description into a live planning board.

The renderer is dependency-free and **escapes all input**; link URLs are limited to `http(s)`/`mailto` — no untrusted HTML or `javascript:` links can run in the app.

### Deleting a project

From **Settings → Danger zone** you get two levels:

- **Remove from Feather** — deletes the project (and its issues, commits and history) for the team, but **keeps everyone's local files**.
- **Delete everywhere** — also deletes the linked **local folder on every teammate's machine** the next time their Feather launches. This is recorded as a tombstone so late-syncing devices still act on it. A safety guard refuses to delete shallow paths (never a filesystem root or bare home directory). Permanent.

### Multi-device sync

Every deploy writes a small state marker to the server. Other Feather installations that have the same project bound to a local folder poll it and automatically pull new deploys into their folder — as long as their working tree is clean (local uncommitted changes are never overwritten).

### Auto-updates

Feather ships with a built-in updater fed by GitHub releases. When a new version is published it offers to download and install it; releases are cryptographically signed and only signed updates are accepted.

---

## The storage backend

Commit snapshots and rollback states are the only file bytes Feather stores in the cloud, and they do **not** go to Supabase Storage. Instead they live on a dedicated **Pterodactyl server**, reached exclusively through the **`feather-storage`** Supabase Edge Function:

- **The key stays server-side.** The storage server's Pterodactyl API key lives only in the function's secret store (`FEATHER_STORAGE_KEY`) — never in the app, never in the repo. The desktop app calls the function; the function calls the panel.
- **The function authorizes every call.** It verifies the caller's Supabase session, confirms they're on the team that owns the referenced project (through Row-Level Security), and **derives the storage path itself** from the ids — `data/<team>/<project>/commits/<commit>.zip` — so a caller can never reach another team's files or the rest of the server. It creates the folder tree on first write.
- **The storage server is hard-excluded from normal use.** Feather's Rust core filters that specific server out of every server listing and rejects every server-scoped operation against it (details, resources, power, console, files, backups, deploy). Even a user who connects a panel at the same host with a key that can see it can never list, import, browse or deploy to it.

See [`supabase/functions/feather-storage/README.md`](supabase/functions/feather-storage/README.md) to deploy it and set the secret.

---

## Security & privacy

- **API keys are encrypted at rest** in the cloud (a master key in Supabase Vault) and only ever decrypted for verified team members through database functions that check membership first.
- **The storage server's key is never in the app.** It lives only in the `feather-storage` Edge Function; the app talks to the function, which authorizes each request and builds every path server-side. The storage server is excluded from all normal Feather server operations.
- **Keys are never written to local disk.** On your device the decrypted panel keys live in memory only, for the session.
- **Row-Level Security** is enabled on every table: you only read or write data for teams you belong to. Sensitive actions (creating teams, encrypting/decrypting keys, inviting members, changing roles, recording commits/deploys, opening issues, deleting projects) go through `SECURITY DEFINER` database functions that re-check permissions and stamp the acting user server-side, so the client can't forge them.
- **Supabase never receives your project files** — only metadata. Deploys read local files and talk to your panel directly; commit snapshots go to the storage backend through the function.
- **The anon/public key and Project URL** shipped in the app are safe to embed by design; the database enforces access, not secrecy of those values. The service-role key and database password must never go into the app.

---

## Development

Prerequisites: [Rust](https://rustup.rs), Node.js ≥ 20, and on Linux the [Tauri system dependencies](https://tauri.app/start/prerequisites/) (webkit2gtk 4.1, GTK 3).

```sh
npm install
npm run tauri dev
```

The collaboration features need the cloud backend configured (see [First-time setup](#first-time-setup)); the server/deploy features work against any Pterodactyl panel. No panel at hand? Run the bundled mock:

```sh
cargo run -p mock-panel
# → panel URL http://127.0.0.1:8899, API key printed on startup
```

### Repository layout

| Path | Contents |
|---|---|
| `crates/feather-core` | Panel API client, config, git, the deploy engine, commit **snapshots/diffs** and the reserved-**storage** guard — no Tauri dependency, fully testable headless |
| `crates/mock-panel` | A mock of the Pterodactyl client API for tests and local development |
| `src-tauri` | Tauri 2 shell: window, IPC commands, multiple in-memory panel connections, per-device project-folder bindings |
| `src` | Svelte 5 + TypeScript frontend (UI, Supabase client, cloud helpers, Markdown renderer, manifest diff) |
| `supabase/` | SQL migrations (`0001`–`0011`) — schema, Row-Level Security and functions |
| `supabase/functions/feather-storage` | The Edge Function that fronts the storage backend (holds the key) |
| `docs/CLOUD-SETUP.md` | Step-by-step cloud backend setup |
| `docs/RELEASING.md` | Release & updater-signing process |
| `docs/SPEC.md` | Product specification (German) |

### Database migrations

The cloud schema is a set of ordered SQL files in [`supabase/`](supabase/), applied once in the Supabase SQL editor:

| File | Adds |
|---|---|
| `0001_foundation.sql` | Profiles, teams, members, encrypted panels, projects, RLS |
| `0002_team_create_rpc.sql` | Reliable team creation |
| `0003_fix_panel_crypto.sql` | Points panel encryption at the `extensions` schema |
| `0004_team_members.sql` | Invite / remove members by email |
| `0005_deploys.sql` | Deploy-history table and recorder |
| `0006_issues.sql` | Issues, comments and their functions |
| `0007_project_deletions.sql` | "Delete everywhere" tombstones |
| `0008_profiles.sql` | User/team profile fields; owner-only team edits; `set_member_role` (admin rights) |
| `0009_commits.sql` | Cloud commits & deploy bundles (metadata) and their RPCs |
| `0010_commit_manifests.sql` | Per-commit/per-deploy manifests for diffs; `finalize_commit` / `server_manifest` |
| `0011_issue_links.sql` | Link issues to the current Deploy and the fixing commit |

All are idempotent — safe to re-run. Cloud commits also need the [`feather-storage`](supabase/functions/feather-storage/README.md) function deployed.

### Tests

```sh
cargo test        # core + mock panel (integration tests run against the mock)
npm run check     # svelte-check (types)
npm test          # vitest (formatting, Markdown renderer incl. XSS cases, manifest diff)
```

---

## License

[MIT](LICENSE)
