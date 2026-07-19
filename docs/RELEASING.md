# Releasing Wingman

## One-time setup (updater signing key)

The auto-updater only installs updates signed with the project's key.

1. Generate the keypair (pick a password or leave it empty):

   ```sh
   npm run tauri signer generate -- -w ~/.tauri/wingman.key
   ```

2. Put the **public key** (contents of `~/.tauri/wingman.key.pub`) into
   `src-tauri/tauri.conf.json` under `plugins.updater.pubkey` and commit it.

3. Add two GitHub Actions secrets (repo → Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/wingman.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password (empty if none)

   Keep `~/.tauri/wingman.key` safe. Losing it means shipped apps cannot
   verify future updates and users must reinstall manually.

## Every release

1. Bump the version in `Cargo.toml` (workspace), `package.json` and
   `src-tauri/tauri.conf.json` — keep all three identical.
2. Update the changelog section in the GitHub release notes (the release
   workflow creates a draft you can edit).
3. Tag and push:

   ```sh
   git tag v0.5.0
   git push origin v0.5.0
   ```

4. The **Release** workflow builds:
   - Windows: NSIS installer (`Wingman_…_x64-setup.exe`)
   - Linux: `.deb` and `.AppImage`
   - `latest.json` + signatures for the auto-updater
5. Review the draft release on GitHub and **publish** it. From that moment
   the built-in updater offers the new version to existing installations,
   and `install.sh` picks it up.
