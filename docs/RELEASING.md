# Releasing Feather

## One-time setup (updater signing key)

The auto-updater only installs updates signed with the project's key.

1. Generate the keypair (pick a password or leave it empty):

   ```sh
   npm run tauri signer generate -- -w ~/.tauri/feather.key
   ```

2. Put the **public key** (contents of `~/.tauri/feather.key.pub`) into
   `src-tauri/tauri.conf.json` under `plugins.updater.pubkey` and commit it.

3. Add two GitHub Actions secrets (repo → Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/feather.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password (empty if none)

   Keep `~/.tauri/feather.key` safe. Losing it means shipped apps cannot
   verify future updates and users must reinstall manually.

## Every release

1. Bump the version in `Cargo.toml` (workspace), `package.json` and
   `src-tauri/tauri.conf.json` — keep all three identical.
2. Update `CHANGELOG.md`: turn the top section into the new version with
   today's date. The release notes on GitHub can reuse it.
3. If the release adds or changes cloud features, ship any new SQL migration
   in `supabase/` (and any change to the `supabase/functions/feather-storage`
   Edge Function) and note in the release that users must run/redeploy them
   (see `docs/CLOUD-SETUP.md`). Migrations are idempotent and run by the user
   in the Supabase SQL editor; the Edge Function is deployed with the Supabase
   CLI — neither is part of the built app.
4. Tag and push:

   ```sh
   git tag v2.5.0
   git push origin v2.5.0
   ```

4. The **Release** workflow builds:
   - Windows: NSIS installer (`Feather_…_x64-setup.exe`)
   - Linux: `.deb` and `.AppImage`
   - `latest.json` + signatures for the auto-updater
5. Review the draft release on GitHub and **publish** it. From that moment
   the built-in updater offers the new version to existing installations,
   and `install.sh` picks it up.
