// Typed helpers for Feather's cloud data (Supabase). More will be added per
// milestone (panels, projects, deploys, issues).

import { supabase } from "./supabase";

export interface Team {
  id: string;
  name: string;
  created_at: string;
}

export async function listTeams(): Promise<Team[]> {
  const { data, error } = await supabase
    .from("teams")
    .select("id, name, created_at")
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return data ?? [];
}

export async function createTeam(name: string): Promise<Team> {
  // Inserts server-side via a SECURITY DEFINER function (supabase/0002),
  // which stamps owner_id from auth.uid() and is not subject to the teams
  // INSERT policy — the reliable way to create a team.
  const { data, error } = await supabase.rpc("create_team", { p_name: name.trim() });
  if (error) throw new Error(error.message);
  if (!data) throw new Error("team was not created");
  const team = Array.isArray(data) ? data[0] : data;
  return { id: team.id, name: team.name, created_at: team.created_at };
}

// --- Panels (Pterodactyl connections, shared in the team) ------------------
//
// The API key is never selected from this table — the `api_key_encrypted`
// column stays server-side. Metadata (name, URL) is readable by team members;
// the plaintext key is only reachable through the panelApiKey() RPC, which
// decrypts it after checking membership.

export interface CloudPanel {
  id: string;
  name: string;
  base_url: string;
  created_at: string;
}

export async function listPanels(teamId: string): Promise<CloudPanel[]> {
  const { data, error } = await supabase
    .from("panels")
    .select("id, name, base_url, created_at")
    .eq("team_id", teamId)
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return data ?? [];
}

/**
 * Store a new panel. The key is encrypted server-side by create_panel() and
 * never travels back; we re-read the row to return its metadata.
 */
export async function createPanel(
  teamId: string,
  name: string,
  baseUrl: string,
  apiKey: string,
): Promise<CloudPanel> {
  const { data: id, error } = await supabase.rpc("create_panel", {
    p_team: teamId,
    p_name: name,
    p_base_url: baseUrl,
    p_api_key: apiKey,
  });
  if (error) throw new Error(error.message);
  const { data, error: readError } = await supabase
    .from("panels")
    .select("id, name, base_url, created_at")
    .eq("id", id)
    .single();
  if (readError) throw new Error(readError.message);
  return data;
}

export async function deletePanel(panelId: string): Promise<void> {
  const { error } = await supabase.from("panels").delete().eq("id", panelId);
  if (error) throw new Error(error.message);
}

/** Decrypt a panel's API key — team members only (SECURITY DEFINER RPC). */
export async function panelApiKey(panelId: string): Promise<string> {
  const { data, error } = await supabase.rpc("panel_api_key", { p_panel: panelId });
  if (error) throw new Error(error.message);
  if (typeof data !== "string") throw new Error("could not decrypt panel key");
  return data;
}
