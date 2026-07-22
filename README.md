# Feather

**A cloud-backed, team-collaborative desktop client for [Pterodactyl](https://pterodactyl.io) — think GitHub Desktop crossed with GitHub itself, but for your game servers, bots and self-hosted services.**

Feather pairs a local one-click **deploy engine** (pick a folder, version it with git, push it to your Pterodactyl server) with a shared **cloud workspace** (accounts, teams, encrypted panels, projects, deploy history, issues and Markdown planning) so a whole team can build, deploy, plan and review together. The name plays on Pterodactyl's flight theme — its daemon is called [Wings](https://github.com/pterodactyl/wings).

> **Status:** v2.1 — feature-complete for both the local deploy workflow (panel connection, live servers, one-click deploys, git-backed versioning with rollback, file browser, auto-updater) **and** the cloud collaboration layer (accounts, teams, members, encrypted shared panels, cloud projects, deploy history, issues, Markdown planning).

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
  - [Server dashboard](#server-dashboard)
  - [Projects](#projects)
  - [The project page](#the-project-page)
  - [Deploying](#deploying)
  - [Versioning & rollback](#versioning--rollback)
  - [Deploy history](#deploy-history)
  - [Issues](#issues)
  - [Markdown planning & checklists](#markdown-planning--checklists)
  - [Multi-device sync](#multi-device-sync)
  - [Auto-updates](#auto-updates)
- [Security & privacy](#security--privacy)
- [Development](#development)
- [License](#license)

---

## What Feather does

Feather has two halves that work together:

**🚀 The local deploy engine** runs on each teammate's own machine. It reads a local project folder, keeps its history as real git commits, packs it up and pushes it to a Pterodactyl server through the panel's file API — while letting you manage the servers (power, console, live stats, files) right next to it.

**☁️ The cloud workspace** is a free [Supabase](https://supabase.com) backend that a team shares. It stores accounts, teams, encrypted Pterodactyl panel keys, projects (with descriptions and planning), the full deploy history, and per-project issues. Everyone on a team sees the same data in real time; nobody has to re-enter API keys or lose track of who deployed what.

At a glance:

| Area | What you get |
|---|---|
| **Accounts & teams** | Email sign-in; a team is the shared unit for everything below |
| **Members** | Invite teammates by email, assign the work, protected owner |
| **Panels** | Multiple Pterodactyl connections per team, API keys **encrypted at rest** and shared — connect and deploy without re-entering keys |
| **Server dashboard** | Live tiles: status, CPU, RAM, power actions, streamed console, file browser |
| **Projects** | A shared, GitHub-style catalog of everything you work on, each with its own page |
| **Deploy** | One-click zip → upload → extract, `.deployignore`, target folder, post-deploy restart/notify |
| **Versioning** | Every project is a git repo; commit, view history, one-click rollback |
| **Deploy history** | A shared timeline per project — status, commit, file count, who & when |
| **Issues** | Per-project issue tracker with comments, open/closed, numbering |
| **Planning** | Markdown descriptions, issues & comments; interactive `- [ ]` checklists |
| **Comfort** | Multi-device sync, built-in auto-updater |

---

## How it fits together

```
   ┌─────────────────────────────┐         ┌────────────────────────────┐
   │   Your machine (Feather)     │         │   Supabase (shared cloud)   │
   │                              │  auth   │                            │
   │  Svelte UI  ── in-memory ──▶ │◀───────▶│  accounts · teams · members │
   │      │        panel key      │  data   │  encrypted panel keys       │
   │      ▼                       │◀───────▶│  projects · descriptions    │
   │  Rust deploy engine          │         │  deploy history · issues    │
   │  (git · zip · upload)        │         │  (Row-Level Security)       │
   └──────────┬───────────────────┘         └────────────────────────────┘
              │ Pterodactyl client API (files, power, websocket)
              ▼
       ┌──────────────┐
       │  Your panel  │
       └──────────────┘
```

The **deploy engine can only run locally** — it needs your files, git and a direct line to the panel — so it stays on each machine. The **cloud never sees your files or your plaintext API keys**; it holds only the shared metadata that makes teamwork possible. When you connect a panel, Feather fetches its key from the cloud, decrypts it for you (a team member), and hands it to the Rust engine **in memory only** — it is never written to local disk.

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
3. Run the SQL migrations in [`supabase/`](supabase/) (`0001` … `0006`) in the SQL editor — they create every table, security policy and function.
4. Turn on email login.
5. Copy your **Project URL** and **anon public key** into `src/lib/supabase.ts` (or hand them to whoever builds the app).

These two values are meant to live inside the app; your data is protected by database Row-Level Security, not by keeping them secret. **Never** share the service-role key or database password.

### 2. Create an account and sign in

Launch Feather. On first run you'll see the sign-in screen — create an account with your email and a password, then sign in. (If your Supabase project has "Confirm email" on, confirm via the email link first.)

### 3. Create or join a team

A **team** is where everything is shared. Create one (you become its owner), or — if a teammate has already added you — pick an existing team. You can switch teams any time from the header menu.

### 4. Add a Pterodactyl panel

Open the **Panels** tab → **Add panel**. In your Pterodactyl account, create a client API key under **Account → API Credentials**, paste in the panel URL and key, optionally **Test** it, then **Add**. The key is encrypted in the cloud and shared with your team. Click **Connect** to open its server dashboard.

### 5. Deploy your first project

On the dashboard, a server tile shows **Link project…** — pick a local folder, and Feather imports the server's current files into it and makes the first git checkpoint. From then on, **Deploy** pushes your folder to the server in one click, and the deploy is recorded in the project's history for the whole team.

---

## Feature reference

### Accounts & sign-in

Feather is account-based. Sign up with an email, display name and password; sign in on any machine to reach your teams. Sessions persist across restarts and refresh automatically. Log out from the header menu (top-right).

### Teams

A team is the unit of collaboration — its panels, projects, deploy history and issues are shared by everyone on it. You can belong to several teams and switch between them from the header (**Switch team**). Database Row-Level Security guarantees you only ever see teams you belong to. The creator of a team is its **owner**.

### Members

The **Members** tab lists everyone on the team with their name, handle and role (**owner** / **admin** / **member**). Owners and admins can:

- **Add a member** by email — the person must already have a Feather account with that email.
- **Remove a member** — the team owner is always protected and can't be removed.

New members immediately share the team's panels, projects and history.

### Panels (encrypted & shared)

A panel is a connection to a Pterodactyl installation. Under **Panels** you can add, connect to, and remove panels; a team can have several.

- **Encrypted at rest.** API keys are encrypted with a master key kept in Supabase Vault and can only be decrypted by team members, through a database function that checks membership first. The raw key never leaves the database in plaintext.
- **Shared.** Every teammate can connect and deploy without re-entering the key.
- **In-memory on your device.** When you connect a panel, the decrypted key is held in RAM for the session only and handed to the local engine — it is never written to local disk. Disconnecting (or closing the app) drops it.

### Server dashboard

Connecting a panel opens its dashboard — one tile per server with:

- **Live status, CPU and RAM**, streamed over the Wings websocket (with token refresh and auto-reconnect).
- **Power actions** — start, stop, restart, and a two-click **kill**.
- **Console** — streamed live output with a command input.
- **Files** — a server file browser: navigate directories, create folders, delete files.
- **Deploy / project** actions when a project is linked (see below).

### Projects

The **Projects** tab is the team's home — a shared catalog of everything you're working on. Each project has a name and a description and can be linked to a panel and server. Create, rename, describe and delete projects; every change syncs to the whole team. Projects are also created automatically the first time you deploy a server that doesn't have one yet, so nothing goes untracked.

### The project page

Clicking a project opens its GitHub-style page with a tab bar:

- **Overview** — the rendered [Markdown](#markdown-planning--checklists) description (goals, plans, interactive checklists) plus a sidebar: linked panel, server, deploy target, post-deploy behaviour, and who created it when.
- **Issues** — the project's [issue tracker](#issues).
- **Deploys** — the project's [deploy history](#deploy-history).
- **Settings** — edit every field (name, description, panel, server, deploy target directory, build command, post-deploy behaviour, auto-backup) and delete the project.

### Deploying

Linking a folder to a server and pressing **Deploy** runs the local engine:

1. **Commit** the current state of the folder (a git checkpoint).
2. **Build** — optionally run a per-project build command first, with streamed output.
3. **Back up** the server first (optional, on by default).
4. **Pack** the folder into a zip, honouring `.deployignore` (gitignore syntax) so you can exclude paths.
5. **Upload & extract** through the panel's file API into the chosen **target directory** (server root by default, or a subfolder).
6. **Reconcile** — files you deleted locally are removed on the server via a manifest diff, so the server mirrors your folder.
7. **After deploy** — restart the server or just send a desktop notification, per project.

Live progress (`Backing up… · Uploading 68%…`) shows on the server tile, and the outcome is recorded to the project's [deploy history](#deploy-history).

### Versioning & rollback

Every linked project is a **real git repository**, initialized and committed automatically — no git knowledge required, and power users can work with the repo directly.

- **Commit** your working state any time from the UI, with a message.
- **History** shows past commits; the footer tracks "N commits since last deploy".
- **Rollback** re-deploys an older commit with one click **without touching your working tree** — your local files are left exactly as they are.
- **Backups** are made before each deploy and rotated; Feather never deletes backups it didn't create.

### Deploy history

The **Deploys** tab on each project is a shared timeline of every deploy and rollback the team has run: ✓/✕ status, the git commit and its summary, how many files, who ran it, and when. Deploys and rollbacks from the server dashboard are recorded automatically (recording is best-effort and never interrupts a deploy). The whole team sees the same history.

### Issues

Each project has a GitHub-style **issue tracker**:

- **Open an issue** with a title and Markdown description; issues are numbered per project (#1, #2, …).
- **Discuss** in comments.
- **Close / reopen** issues; filter by **Open** / **Closed**; see comment counts at a glance.

Everything is shared across the team, and comments are attributed to their author.

### Markdown planning & checklists

Project descriptions, issue bodies and comments render **Markdown** — headings, bold/italic, lists, inline code and code blocks, blockquotes, links, and GitHub-style **task lists**. Checklists in a project's description are **interactive**: tick `- [ ]` items right on the Overview and the change is saved for the whole team, turning the description into a live planning board.

The renderer is dependency-free and **escapes all input**, and link URLs are restricted to `http(s)`/`mailto` — no untrusted HTML or `javascript:` links can ever run inside the app.

### Multi-device sync

Every deploy writes a small state marker to the server. Other Feather installations connected to the same panel poll it and automatically pull new deploys into their local project folder — as long as their working tree is clean (local uncommitted changes are never overwritten). Linking a project to a server with an empty local folder imports the server's current files first, so a teammate can start from the current state in one step.

### Auto-updates

Feather ships with a built-in updater fed by GitHub releases. When a new version is published it offers to download and install it; releases are cryptographically signed and only signed updates are accepted.

---

## Security & privacy

- **API keys are encrypted at rest** in the cloud (a master key in Supabase Vault) and only ever decrypted for verified team members through database functions that check membership first.
- **Keys are never written to local disk.** On your device the decrypted key lives in memory only, for the session.
- **Row-Level Security** is enabled on every table: you can only read or write data for teams you belong to. Sensitive actions (creating teams, encrypting/decrypting keys, inviting members, recording deploys, opening issues) go through `SECURITY DEFINER` database functions that re-check permissions and stamp the acting user server-side, so the client can't forge them.
- **The cloud never receives your files** — deploys read local files and talk to your panel directly.
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
| `src-tauri` | Tauri 2 shell: window, IPC commands, in-memory active-panel state |
| `src` | Svelte 5 + TypeScript frontend (UI, Supabase client, cloud helpers, Markdown renderer) |
| `supabase/` | SQL migrations (`0001`–`0006`) — schema, Row-Level Security and functions |
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
