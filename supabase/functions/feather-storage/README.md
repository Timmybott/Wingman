# `feather-storage` Edge Function

The storage proxy for Feather's cloud commits and rollbacks. It is the **only**
place that ever holds the storage server's Pterodactyl API key — the desktop app
calls this function, never the panel directly, so the key never ships in the
app. See the header of [`index.ts`](./index.ts) for how it authenticates,
authorizes and derives paths.

## What it stores, and where

Files land on the storage server under a fixed, per-team layout that the
function derives itself from ids (the client never passes a path):

```
<STORAGE_ROOT>/<team_id>/<project_id>/commits/<commit_id>.zip
<STORAGE_ROOT>/<team_id>/<project_id>/rollbacks/<commit_id>.zip
```

The folder tree is created on first write, so you do **not** need to create
`data/` by hand. Nginx and the rest of the server are never touched.

## One-time setup

You need the [Supabase CLI](https://supabase.com/docs/guides/cli) linked to your
project (`supabase link`).

1. **Set the secret** (the storage server's Pterodactyl **client** API key):

   ```sh
   supabase secrets set FEATHER_STORAGE_KEY=ptlc_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
   ```

   Optional overrides (defaults shown) — only set these if they differ:

   ```sh
   supabase secrets set STORAGE_PANEL_URL=https://panel.spaceify.eu/
   supabase secrets set STORAGE_SERVER_ID=893a2ffd
   supabase secrets set STORAGE_ROOT=data          # base dir on the server
   ```

   `SUPABASE_URL` and `SUPABASE_ANON_KEY` are injected by the runtime — do not
   set them.

2. **Deploy the function:**

   ```sh
   supabase functions deploy feather-storage
   ```

That's it. Until `FEATHER_STORAGE_KEY` is set the function returns `503` and
Feather treats cloud storage as unavailable, so deploying it early is harmless.

## Security notes

- The key lives only in Supabase's secret store, server-side. It is never
  returned to the client and never logged.
- Every request is authorized with the caller's own JWT: the function reads the
  referenced project through Row-Level Security, so a user who is not on the
  owning team cannot read or write anything.
- Paths are built from ids on the server; a client cannot ask for an arbitrary
  path, so it can never reach another team's files or the rest of the server.
- The storage server itself (`STORAGE_SERVER_ID` on `STORAGE_PANEL_URL`) is also
  hard-excluded from every normal Feather server path in the desktop app, so it
  never appears as an ordinary server.
