# Wingman

**A desktop client for [Pterodactyl](https://pterodactyl.io) with a one-click deploy workflow — think GitHub Desktop, but for your game servers.**

> ⚠️ Wingman is in early development. Milestones M1–M4 (panel connection, live servers, one-click deploys, git-backed versioning with rollback) are implemented; the file browser, auto-updater and first releases (M5) are on the way. The name is a nod to Pterodactyl's daemon, [Wings](https://github.com/pterodactyl/wings).

## What it does (and will do)

Pick a local project folder, keep versions as git commits, and push them to your Pterodactyl server with a single click — while managing the servers right next to it.

- ✅ **M1 — Connection & dashboard**: connect a panel with a client API key (stored in the OS keychain), see your servers as tiles with live status, CPU and RAM
- ✅ **M2 — Live servers**: power actions (start/stop/restart/kill), streamed console with command input, live stats over the Wings websocket (token refresh + auto-reconnect included)
- ✅ **M3 — Deploy**: link a project folder and deploy it with one click — zip → upload → extract via the panel's file API, `.deployignore` (gitignore syntax), stale files deleted via manifest diff, configurable target folder, post-deploy restart or desktop notification, live progress on the server tile
- ✅ **M4 — Versioning**: every project is a real git repository (auto-initialized, auto-committed on deploy), commit UI and history with one-click rollback that never touches your working tree, automatic pre-deploy backups with rotation (Wingman never deletes backups it didn't create), optional build command with streamed output
- 🔜 **M5 — Comfort & release**: server file browser, auto-updater, first Windows + Linux releases

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

## Security

API keys are stored exclusively in the operating system's keychain (Windows
Credential Manager / Secret Service on Linux) — never in plain-text files.

## License

[MIT](LICENSE)
