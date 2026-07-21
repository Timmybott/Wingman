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
  const { data: userData } = await supabase.auth.getUser();
  const ownerId = userData.user?.id;
  if (!ownerId) throw new Error("not signed in");
  const { data, error } = await supabase
    .from("teams")
    .insert({ name: name.trim(), owner_id: ownerId })
    .select("id, name, created_at")
    .single();
  if (error) throw new Error(error.message);
  return data;
}
