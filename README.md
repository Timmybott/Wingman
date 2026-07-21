# Feather

**A desktop client for [Pterodactyl](https://pterodactyl.io) with a one-click deploy workflow — think GitHub Desktop, but for your game servers.**

> ⚠️ Feather is young software, but feature-complete for v1: all five milestones (panel connection, live servers, one-click deploys, git-backed versioning with rollback, file browser + auto-updater) are implemented. The name plays on Pterodactyl's flight theme — its daemon is called [Wings](https://github.com/pterodactyl/wings).

## Installation

> The first public release is in preparation — the buttons below go live with it.

**Windows:** Download `Feather_x.y.z_x64-setup.exe` from the
[latest release](https://github.com/Timmybott/Wingman/releases/latest) and run it.

**Linux:** One line — installs the `.deb` on Debian/Ubuntu, the AppImage everywhere else:

```sh
curl -fsSL https://raw.githubusercontent.com/Timmybott/Wingman/main/install.sh | bash
```

Once installed, Feather keeps itself up to date through its built-in updater.

## What it does (and will do)

Pick a local project folder, keep versions as git commits, and push them to your Pterodactyl server with a single click — while managing the servers right next to it.

- ✅ **M1 — Connection & dashboard**: connect a panel with a client API key (stored in the OS keychain), see your servers as tiles with live status, CPU and RAM
- ✅ **M2 — Live servers**: power actions (start/stop/restart/kill), streamed console with command input, live stats over the Wings websocket (token refresh + auto-reconnect included)
- ✅ **M3 — Deploy**: link a project folder and deploy it with one click — zip → upload → extract via the panel's file API, `.deployignore` (gitignore syntax), stale files deleted via manifest diff, configurable target folder, post-deploy restart or desktop notification, live progress on the server tile
- ✅ **M4 — Versioning**: every project is a real git repository (auto-initialized, auto-committed on deploy), commit UI and history with one-click rollback that never touches your working tree, automatic pre-deploy backups with rotation (Feather never deletes backups it didn't create), optional build command with streamed output
- ✅ **M5 — Comfort & release**: server file browser (navigate, create folders, delete), built-in auto-updater fed by GitHub releases, release pipeline for Windows (NSIS) + Linux (deb/AppImage) and a one-line Linux installer

## Development

Prerequisites: [Rust](https://rustup.rs), Node.js ≥ 20, and on Linux the
[Tauri system dependencies](https://tauri.app/start/prerequisites/) (webkit2gtk 4.1, GTK 3).

```sh
npm install
npm run tauri dev
```

No panel at hand? Run the bundled mock panel and connect to it:

```sh
cargo run -p mock-panel
# → panel URL http://127.0.0.1:8899, API key is printed on startup
```

### Repository layout

| Path | Contents |
|---|---|
| `crates/wingman-core` | Panel API client, config, deploy engine — no Tauri dependency, fully testable headless |
| `crates/mock-panel` | Mock of the Pterodactyl client API for tests and local development |
| `src-tauri` | Tauri 2 shell: windows, IPC commands, OS keychain access |
| `src` | Svelte 5 frontend |
| `docs/SPEC.md` | Product specification (German) |

### Tests

```sh
cargo test        # core + mock panel (integration tests run against the mock)
npm run check     # svelte-check
npm test          # vitest
```

## Multi-device sync

Every deploy writes a small state marker to the server. Other Feather
installations connected to the same panel poll it and automatically pull new
deploys into their local project folder — as long as their working tree is
clean. Local uncommitted changes are never overwritten. Linking a project to
a server with an empty local folder imports the server's current files first.

## Security

API keys are stored in the operating system's keychain (Windows Credential
Manager / Secret Service on Linux). If no keychain is available — for example
a minimal Linux setup without GNOME Keyring or KWallet — the key falls back
to an obfuscated (not encrypted) file in Feather's config directory so the
app stays usable; install a Secret Service if you want keychain-grade
protection.

## License

[MIT](LICENSE)
