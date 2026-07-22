// Feather storage proxy (Supabase Edge Function).
//
// Feather stores commit snapshots and rollback states as files on a dedicated
// Pterodactyl server. This function is the ONLY thing that ever holds that
// server's API key: the desktop app calls this function, never the panel
// directly, so the key stays server-side and is never shipped in the app.
//
// What it does on every request:
//   1. Authenticates the caller with their Supabase JWT.
//   2. Confirms they are a member of the team that owns the referenced project
//      (Row-Level Security on `projects` does the check — a non-member simply
//      can't see the row).
//   3. Derives the storage path *itself* from the ids (the client never passes
//      raw paths), so a caller can only ever touch their own team's area:
//        <STORAGE_ROOT>/<team_id>/<project_id>/<kind>s/<commit_id>.zip
//   4. Performs the file op against the Pterodactyl client API, creating the
//      folder tree on first write. Nginx and the rest of the server are never
//      touched.
//
// Deploy + configuration: see supabase/functions/feather-storage/README.md.
//
// Required secrets (supabase secrets set …):
//   FEATHER_STORAGE_KEY   Pterodactyl client API key for the storage server.
// Optional (have sensible defaults):
//   STORAGE_PANEL_URL     default https://panel.spaceify.eu/
//   STORAGE_SERVER_ID     default 893a2ffd
//   STORAGE_ROOT          default data   (base dir on the server)
// Provided automatically by the Supabase runtime:
//   SUPABASE_URL, SUPABASE_ANON_KEY

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const PANEL_URL = (Deno.env.get("STORAGE_PANEL_URL") ?? "https://panel.spaceify.eu/").replace(
  /\/?$/,
  "/",
);
const SERVER_ID = Deno.env.get("STORAGE_SERVER_ID") ?? "893a2ffd";
const ROOT = (Deno.env.get("STORAGE_ROOT") ?? "data").replace(/^\/+|\/+$/g, "");
const STORAGE_KEY = Deno.env.get("FEATHER_STORAGE_KEY") ?? "";

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, content-type",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { ...CORS, "content-type": "application/json" },
  });
}

/** A Pterodactyl client-API call against the storage server. */
async function ptero(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${STORAGE_KEY}`);
  headers.set("Accept", "application/json");
  return await fetch(new URL(path, PANEL_URL), { ...init, headers });
}

/** Create `dir` and every parent under the server root; ignore "exists". */
async function ensureDir(dir: string): Promise<void> {
  const parts = dir.split("/").filter(Boolean);
  let root = "/";
  for (const name of parts) {
    const res = await ptero(`api/client/servers/${SERVER_ID}/files/create-folder`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ root, name }),
    });
    // 204 = created; a 4xx here is almost always "already exists" — fine.
    if (!res.ok && res.status >= 500) {
      throw new Error(`create-folder ${root}${name} failed: ${res.status}`);
    }
    root = root === "/" ? `/${name}` : `${root}/${name}`;
  }
}

Deno.serve(async (req) => {
  if (req.method === "OPTIONS") return new Response("ok", { headers: CORS });
  if (STORAGE_KEY === "") return json({ error: "storage backend not configured" }, 503);

  const url = new URL(req.url);
  const action = url.searchParams.get("action") ?? "";
  const projectId = url.searchParams.get("project_id") ?? "";
  const commitId = url.searchParams.get("commit_id") ?? "";
  const kind = url.searchParams.get("kind") ?? "commit"; // commit | rollback

  if (!UUID.test(projectId)) return json({ error: "bad project_id" }, 400);
  if (kind !== "commit" && kind !== "rollback") return json({ error: "bad kind" }, 400);
  if (action !== "list" && !UUID.test(commitId)) return json({ error: "bad commit_id" }, 400);

  // 1 + 2. Authenticate and authorize via the caller's token + RLS.
  const authHeader = req.headers.get("Authorization") ?? "";
  if (!authHeader.startsWith("Bearer ")) return json({ error: "missing token" }, 401);
  const supabase = createClient(
    Deno.env.get("SUPABASE_URL")!,
    Deno.env.get("SUPABASE_ANON_KEY")!,
    { global: { headers: { Authorization: authHeader } } },
  );
  const { data: project, error } = await supabase
    .from("projects")
    .select("team_id")
    .eq("id", projectId)
    .single();
  if (error || !project) return json({ error: "not authorized for this project" }, 403);

  // 3. Derive the path — the client never supplies one.
  const dir = `${ROOT}/${project.team_id}/${projectId}/${kind}s`;
  const file = `/${dir}/${commitId}.zip`;

  try {
    // 4. Perform the file op.
    switch (action) {
      case "put": {
        await ensureDir(dir);
        const body = new Uint8Array(await req.arrayBuffer());
        const res = await ptero(
          `api/client/servers/${SERVER_ID}/files/write?file=${encodeURIComponent(file)}`,
          { method: "POST", headers: { "content-type": "application/octet-stream" }, body },
        );
        if (!res.ok) return json({ error: `write failed: ${res.status}` }, 502);
        return json({ ok: true, bytes: body.byteLength });
      }
      case "get": {
        const res = await ptero(
          `api/client/servers/${SERVER_ID}/files/contents?file=${encodeURIComponent(file)}`,
        );
        if (!res.ok) return json({ error: `read failed: ${res.status}` }, res.status);
        return new Response(res.body, {
          status: 200,
          headers: { ...CORS, "content-type": "application/octet-stream" },
        });
      }
      case "delete": {
        const res = await ptero(`api/client/servers/${SERVER_ID}/files/delete`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ root: `/${dir}`, files: [`${commitId}.zip`] }),
        });
        if (!res.ok) return json({ error: `delete failed: ${res.status}` }, 502);
        return json({ ok: true });
      }
      case "list": {
        const res = await ptero(
          `api/client/servers/${SERVER_ID}/files/list?directory=${encodeURIComponent(`/${dir}`)}`,
        );
        if (!res.ok) return json({ files: [] });
        return new Response(res.body, {
          status: 200,
          headers: { ...CORS, "content-type": "application/json" },
        });
      }
      default:
        return json({ error: "unknown action" }, 400);
    }
  } catch (e) {
    return json({ error: String(e instanceof Error ? e.message : e) }, 500);
  }
});
