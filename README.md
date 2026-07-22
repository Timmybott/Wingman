# Feather

**A cloud-backed, team-collaborative desktop client for [Pterodactyl](https://pterodactyl.io) — think GitHub Desktop crossed with GitHub itself, but for your game servers, bots and self-hosted services.**

Feather splits cleanly in two: **Panels** is where you run your servers (power, live stats, console across every panel your team uses), and **Projects** is where you plan, track and deploy them (each project imports a server, with its own description, issues, deploy history and a one-click deploy from a local folder). Everything is shared through a free cloud backend, so a whole team works from the same picture. The name plays on Pterodactyl's flight theme — its daemon is called [Wings](https://github.com/pterodactyl/wings).

> **Status:** v2.2 — Panels (all servers of all team panels: power, stats, console) and Projects (import a server, plan it, track issues, deploy from a local folder with git history and a shared deploy timeline), on top of accounts, teams, members and encrypted shared panels.

---

## Contents

- [What Feather does](#what-feather-does)
- [How it fits together](#how-it-fits-together)
- [Installation](#installation)
- [First-time setup](#first-time-setup)
- [Feature reference](#feature-reference)
  - [Accounts & sign-in](#accounts--sign-in)
  - [Teams](#teams)
  - [Members](#members)
  - [Panels (encrypted & shared)](#panels-encrypted--shared)
  - [The Panels tab — your servers](#the-panels-tab--your-servers)
  - [Projects — import a server](#projects--import-a-server)
  - [The project page](#the-project-page)
  - [Local folders (per device)](#local-folders-per-device)
  - [Deploying](#deploying)
  - [Versioning & rollback](#versioning--rollback)
  - [Deploy history](#deploy-history)
  - [Files](#files)
  - [Issues](#issues)
  - [Markdown planning & checklists](#markdown-planning--checklists)
  - [Deleting a project](#deleting-a-project)
  - [Multi-device sync](#multi-device-sync)
  - [Auto-updates](#auto-updates)
- [Security & privacy](#security--privacy)
- [Development](#development)
- [License](#license)

---

## What Feather does

Feather is organised around two tabs:

**🖥️ Panels** — live server operation. Feather connects to **all** of your team's Pterodactyl panels at once and shows **every server**, grouped by panel, with power actions (start/stop/restart/kill), live CPU/RAM and a streamed console. This is the "run it" surface.

**📦 Projects** — planning and deploying. A project **imports one of your servers** and gives it a GitHub-style page: a Markdown description and checklists, an issue tracker, a deploy workflow (deploy from a local folder, git history with rollback, a shared deploy timeline) and a server file browser. This is the "build, plan and ship it" surface.

Both live inside a **team**, backed by a free [Supabase](https://supabase.com) project that everyone on the team shares. Nobody re-enters API keys, and everyone sees the same projects, issues and history.

At a glance:

| Area | What you get |
|---|---|
| **Accounts & teams** | Email sign-in; a team is the shared unit for everything below |
| **Members** | Invite teammates by email; the owner is protected |
| **Panels** | Several Pterodactyl connections per team, API keys **encrypted at rest**, all connected at once |
| **Panels tab** | Every server of every panel: power, live CPU/RAM, streamed console |
| **Projects** | Import a server → its own page for planning, issues, deploys and files |
| **Deploy** | One-click zip → upload → extract from a local folder; `.deployignore`, target dir, restart/notify |
| **Versioning** | Every project folder is a git repo; commit, history, one-click rollback |
| **Deploy history** | A shared timeline per project — status, commit, file count, who & when |
| **Issues** | Per-project tracker with comments, open/closed, numbering |
| **Planning** | Markdown descriptions, issues & comments; interactive `- [ ]` checklists |
| **Delete** | Remove from Feather (keeps local files) or delete everywhere (incl. local folders) |

> **On creating/deleting servers:** Feather uses Pterodactyl's **client API**, which cannot create or delete servers or change their RAM/CPU/disk — only your hosting provider can. So Feather **imports and manages existing servers**; it shows their limits read-only.

---

## How it fits together

```
   ┌──────────────────────────────┐        ┌────────────────────────────┐
   │   Your machine (Feather)      │        │   Supabase (shared cloud)   │
   │                               │  auth  │                            │
   │  Svelte UI ── in-memory ────▶ │◀──────▶│  accounts · teams · members │
   │     │      panel keys (RAM)   │  data  │  encrypted panel keys       │
   │     ▼                         │◀──────▶│  projects · issues          │
   │  Rust core                    │        │  deploy history · tombstones│
   │  (multi-panel · git · deploy) │        │  (Row-Level Security)       │
   │     │  local folder bindings  │        └────────────────────────────┘
   └─────┼─────────────────────────┘
         │ Pterodactyl client API (files, power, websocket) — per panel
         ▼
   ┌──────────────┐  ┌──────────────┐
   │  Panel A     │  │  Panel B     │  …
   └──────────────┘  └──────────────┘
```

The **cloud never sees your files or your plaintext API keys** — it holds only the shared metadata that makes teamwork possible. Panel keys are decrypted for you (a team member) and handed to the Rust core **in memory only**. Which local folder a project deploys from is a **per-device binding** (each teammate picks their own, or none), so your files never leave your machine.

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

Feather needs a cloud backend for the collaboration features. It's a free Supabase project and takes about five minutes — **one person on the team does this once** and shares the two public values with everyone else.

### 1. Set up the cloud backend

Follow **[docs/CLOUD-SETUP.md](docs/CLOUD-SETUP.md)** step by step. In short you will:

1. Create a free Supabase project.
2. Create an encryption secret in Supabase Vault (used to encrypt panel keys).
3. Run the SQL migrations in [`supabase/`](supabase/) (`0001` … `0007`) in the SQL editor — they create every table, security policy and function.
4. Turn on email login.
5. Copy your **Project URL** and **anon public key** into `src/lib/supabase.ts` (or hand them to whoever builds the app).

These two values are meant to live inside the app; your data is protected by database Row-Level Security, not by keeping them secret. **Never** share the service-role key or database password.

### 2. Create an account and sign in

Launch Feather. On first run you'll see the sign-in screen — create an account with your email and a password, then sign in. (If your Supabase project has "Confirm email" on, confirm via the email link first.)

### 3. Create or join a team

A **team** is where everything is shared. Create one (you become its owner), or — if a teammate has already added you — pick an existing team. Switch teams any time from the header menu.

### 4. Add a panel

Open the **Panels** tab → **Manage panels → Add panel**. In your Pterodactyl account, create a client API key under **Account → API Credentials**, paste the panel URL and key, optionally **Test**, then **Add**. The key is encrypted in the cloud and shared with the team, and Feather connects to it automatically. Add as many panels as your team uses — a team needs at least one. Their servers now appear in the Panels tab.

### 5. Import a server as a project

Open the **Projects** tab → **New project**: pick a panel, pick one of its servers (its RAM/CPU/disk are shown), give it a name and description, and optionally **choose a local folder** to deploy from on this device. The project opens with its own Overview, Issues, Deploy and Files tabs.

### 6. Deploy

On the project's **Deploy** tab, use **Import from server** to pull the server's current files into your local folder, then **Deploy** to push your folder back in one click. Every deploy is recorded in the project's shared timeline.

---

## Feature reference

### Accounts & sign-in

Feather is account-based. Sign up with an email, display name and password; sign in on any machine to reach your teams. Sessions persist across restarts and refresh automatically. Log out from the header menu (top-right).

### Teams

A team is the unit of collaboration — its panels, projects, deploy history and issues are shared by everyone on it. You can belong to several teams and switch between them from the header (**Switch team**). Row-Level Security guarantees you only ever see teams you belong to. The team's creator is its **owner**.

### Members

The **Members** tab lists everyone on the team with their name, handle and role (**owner** / **admin** / **member**). Owners and admins can **add a member** by email (they must already have a Feather account) and **remove a member** (the owner is protected). New members immediately share the team's panels, projects and history.

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

Deploying, history and files are **not** here — they belong to the project (see below).

### Projects — import a server

The **Projects** tab is the team's home. **New project** imports one of a panel's servers:

1. Pick a **panel** (required).
2. Pick one of its **servers** — its RAM/CPU/disk limits are shown, and servers already imported are marked so you don't duplicate them.
3. Give it a name and description.
4. Optionally choose a **local folder** to deploy from on this device (add it now or later).

The result is a shared project everyone on the team sees. (You can also make a project purely for planning and cross-server management by leaving the local folder unset.)

### The project page

Clicking a project opens its GitHub-style page with a tab bar:

- **Overview** — the rendered [Markdown](#markdown-planning--checklists) description (goals, plans, interactive checklists), the [local-folder binding](#local-folders-per-device) for this device, and a sidebar (linked panel, server, deploy target, post-deploy behaviour, created by/when).
- **Issues** — the project's [issue tracker](#issues).
- **Deploy** — [deploy](#deploying), import from server, commit, [git history & rollback](#versioning--rollback), and the shared [deploy timeline](#deploy-history).
- **Files** — the server [file browser](#files).
- **Settings** — edit every field (name, description, panel, server, deploy target directory, build command, post-deploy behaviour, auto-backup) and [delete the project](#deleting-a-project).

### Local folders (per device)

Deploying pushes a **local folder** to the server, and that folder is chosen **per device** — each teammate binds their own copy (or none). The binding lives on your machine only; the cloud never learns your paths. Set, change or unlink it under **Overview → Local folder**, or when you create the project. Binding a folder makes it a git repository automatically.

### Deploying

On a project's **Deploy** tab (with a local folder set), **Deploy** runs the local engine:

1. **Commit** the current state of the folder (a git checkpoint).
2. **Build** — optionally run a per-project build command first, with streamed output.
3. **Back up** the server first (optional, on by default).
4. **Pack** the folder into a zip, honouring `.deployignore` (gitignore syntax).
5. **Upload & extract** through the panel's file API into the chosen **target directory** (server root by default, or a subfolder).
6. **Reconcile** — files you deleted locally are removed on the server via a manifest diff, so the server mirrors your folder.
7. **After deploy** — restart the server or just send a desktop notification, per project.

Live progress (`Backing up… · Uploading 68%…`) shows on the tab, and the outcome is recorded to the [deploy history](#deploy-history). **Import from server** does the reverse — pulls the server's files into your folder — which is the safe way to start before your first deploy.

### Versioning & rollback

Every bound project folder is a **real git repository**, initialized and committed automatically — no git knowledge required, and power users can work with the repo directly.

- **Commit** your working state any time, with a message (on the Deploy tab).
- **History** shows past commits with author and time.
- **Rollback** re-deploys an older commit with one click **without touching your working tree**.
- **Backups** are made before each deploy and rotated; Feather never deletes backups it didn't create.

### Deploy history

The **Deploy** tab shows a shared timeline of every deploy and rollback: ✓/✕ status, the git commit and its summary, how many files, who ran it, and when. Recording is best effort and never interrupts a deploy. The whole team sees the same history.

### Files

The **Files** tab is a browser for the project's server: navigate directories, create folders and delete files, straight over the panel's file API. It works whether or not you have a local folder bound.

### Issues

Each project has a GitHub-style **issue tracker**: open an issue with a title and Markdown description (numbered per project — #1, #2, …), discuss in comments, and close/reopen. Filter by **Open**/**Closed** and see comment counts at a glance. Everything is shared, and comments are attributed to their author.

### Markdown planning & checklists

Project descriptions, issue bodies and comments render **Markdown** — headings, bold/italic, lists, inline code and code blocks, blockquotes, links, and GitHub-style **task lists**. Checklists in a project's description are **interactive**: tick `- [ ]` items right on the Overview and the change saves for the whole team, turning the description into a live planning board.

The renderer is dependency-free and **escapes all input**; link URLs are limited to `http(s)`/`mailto` — no untrusted HTML or `javascript:` links can run in the app.

### Deleting a project

From **Settings → Danger zone** you get two levels:

- **Remove from Feather** — deletes the project (and its issues and history) for the team, but **keeps everyone's local files**.
- **Delete everywhere** — also deletes the linked **local folder on every teammate's machine** the next time their Feather launches. This is recorded as a tombstone so late-syncing devices still act on it. A safety guard refuses to delete shallow paths (never a filesystem root or bare home directory). Permanent.

### Multi-device sync

Every deploy writes a small state marker to the server. Other Feather installations that have the same project bound to a local folder poll it and automatically pull new deploys into their folder — as long as their working tree is clean (local uncommitted changes are never overwritten).

### Auto-updates

Feather ships with a built-in updater fed by GitHub releases. When a new version is published it offers to download and install it; releases are cryptographically signed and only signed updates are accepted.

---

## Security & privacy

- **API keys are encrypted at rest** in the cloud (a master key in Supabase Vault) and only ever decrypted for verified team members through database functions that check membership first.
- **Keys are never written to local disk.** On your device the decrypted keys live in memory only, for the session.
- **Row-Level Security** is enabled on every table: you only read or write data for teams you belong to. Sensitive actions (creating teams, encrypting/decrypting keys, inviting members, recording deploys, opening issues, deleting projects) go through `SECURITY DEFINER` database functions that re-check permissions and stamp the acting user server-side, so the client can't forge them.
- **The cloud never receives your files or local paths** — deploys read local files and talk to your panel directly.
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
| `crates/wingman-core` | Panel API client, config, git and the deploy engine — no Tauri dependency, fully testable headless |
| `crates/mock-panel` | A mock of the Pterodactyl client API for tests and local development |
| `src-tauri` | Tauri 2 shell: window, IPC commands, multiple in-memory panel connections, per-device project-folder bindings |
| `src` | Svelte 5 + TypeScript frontend (UI, Supabase client, cloud helpers, Markdown renderer) |
| `supabase/` | SQL migrations (`0001`–`0007`) — schema, Row-Level Security and functions |
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

All are idempotent — safe to re-run.

### Tests

```sh
cargo test        # core + mock panel (integration tests run against the mock)
npm run check     # svelte-check (types)
npm test          # vitest (formatting + Markdown renderer, incl. XSS cases)
```

---

## License

[MIT](LICENSE)
