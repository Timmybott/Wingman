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

// --- Projects (shared planning + deploy metadata) --------------------------
//
// A project is the team's unit of work: a name, a description, and — once a
// panel is connected — a link to the server it deploys to. The local folder
// each teammate deploys from stays on their own machine; only this shared
// definition lives in the cloud.

export type PostDeploy = "restart" | "notify";

export interface CloudProject {
  id: string;
  team_id: string;
  name: string;
  description: string;
  panel_id: string | null;
  server_identifier: string | null;
  target_dir: string;
  build_command: string | null;
  post_deploy: PostDeploy;
  auto_backup: boolean;
  created_by: string | null;
  created_at: string;
}

const PROJECT_COLUMNS =
  "id, team_id, name, description, panel_id, server_identifier, target_dir, build_command, post_deploy, auto_backup, created_by, created_at";

export async function listProjects(teamId: string): Promise<CloudProject[]> {
  const { data, error } = await supabase
    .from("projects")
    .select(PROJECT_COLUMNS)
    .eq("team_id", teamId)
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return data ?? [];
}

export async function getProject(id: string): Promise<CloudProject> {
  const { data, error } = await supabase
    .from("projects")
    .select(PROJECT_COLUMNS)
    .eq("id", id)
    .single();
  if (error) throw new Error(error.message);
  return data;
}

export interface NewProject {
  name: string;
  description?: string;
  panel_id?: string | null;
  server_identifier?: string | null;
}

export async function createProject(teamId: string, input: NewProject): Promise<CloudProject> {
  const { data: userData } = await supabase.auth.getUser();
  const { data, error } = await supabase
    .from("projects")
    .insert({
      team_id: teamId,
      name: input.name.trim(),
      description: input.description?.trim() ?? "",
      panel_id: input.panel_id ?? null,
      server_identifier: input.server_identifier?.trim() || null,
      created_by: userData.user?.id ?? null,
    })
    .select(PROJECT_COLUMNS)
    .single();
  if (error) throw new Error(error.message);
  return data;
}

/** Patch a project. Only the provided fields change. */
export async function updateProject(
  id: string,
  patch: Partial<
    Pick<
      CloudProject,
      | "name"
      | "description"
      | "panel_id"
      | "server_identifier"
      | "target_dir"
      | "build_command"
      | "post_deploy"
      | "auto_backup"
    >
  >,
): Promise<CloudProject> {
  const { data, error } = await supabase
    .from("projects")
    .update(patch)
    .eq("id", id)
    .select(PROJECT_COLUMNS)
    .single();
  if (error) throw new Error(error.message);
  return data;
}

export async function deleteProject(id: string): Promise<void> {
  const { error } = await supabase.from("projects").delete().eq("id", id);
  if (error) throw new Error(error.message);
}

// --- Team members ----------------------------------------------------------

export type TeamRole = "owner" | "admin" | "member";

export interface TeamMember {
  user_id: string;
  role: TeamRole;
  created_at: string;
  display_name: string | null;
  username: string | null;
}

export async function listMembers(teamId: string): Promise<TeamMember[]> {
  const { data, error } = await supabase
    .from("team_members")
    .select("user_id, role, created_at, profiles(display_name, username)")
    .eq("team_id", teamId)
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => {
    // The profiles embed comes back as an object (to-one) but is typed loosely.
    const raw = (row as { profiles?: unknown }).profiles;
    const profile = (Array.isArray(raw) ? raw[0] : raw) as
      | { display_name: string | null; username: string | null }
      | null
      | undefined;
    return {
      user_id: row.user_id,
      role: row.role,
      created_at: row.created_at,
      display_name: profile?.display_name ?? null,
      username: profile?.username ?? null,
    };
  });
}

/** Add an existing Feather account to the team by email (admins only). */
export async function inviteMember(teamId: string, email: string): Promise<void> {
  const { error } = await supabase.rpc("invite_member", {
    p_team: teamId,
    p_email: email.trim(),
  });
  if (error) throw new Error(error.message);
}

export async function removeMember(teamId: string, userId: string): Promise<void> {
  const { error } = await supabase.rpc("remove_member", {
    p_team: teamId,
    p_user: userId,
  });
  if (error) throw new Error(error.message);
}

// --- Deploy history --------------------------------------------------------

export type DeployKind = "deploy" | "rollback" | "sync";
export type DeployStatus = "success" | "failed";

export interface DeployEntry {
  id: string;
  kind: DeployKind;
  status: DeployStatus;
  commit: string | null;
  commit_summary: string | null;
  files_count: number | null;
  message: string | null;
  created_at: string;
  user_id: string | null;
  display_name: string | null;
  username: string | null;
}

export async function listDeploys(projectId: string): Promise<DeployEntry[]> {
  const { data, error } = await supabase
    .from("deploys")
    .select(
      "id, kind, status, commit, commit_summary, files_count, message, created_at, user_id, profiles(display_name, username)",
    )
    .eq("project_id", projectId)
    .order("created_at", { ascending: false });
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => {
    const raw = (row as { profiles?: unknown }).profiles;
    const profile = (Array.isArray(raw) ? raw[0] : raw) as
      | { display_name: string | null; username: string | null }
      | null
      | undefined;
    return {
      id: row.id,
      kind: row.kind,
      status: row.status,
      commit: row.commit,
      commit_summary: row.commit_summary,
      files_count: row.files_count,
      message: row.message,
      created_at: row.created_at,
      user_id: row.user_id,
      display_name: profile?.display_name ?? null,
      username: profile?.username ?? null,
    };
  });
}

export interface DeployOutcome {
  projectId: string;
  kind: DeployKind;
  status: DeployStatus;
  commit?: string | null;
  commitSummary?: string | null;
  files?: number | null;
  message?: string | null;
}

export async function recordDeploy(outcome: DeployOutcome): Promise<void> {
  const { error } = await supabase.rpc("record_deploy", {
    p_project: outcome.projectId,
    p_kind: outcome.kind,
    p_status: outcome.status,
    p_commit: outcome.commit ?? null,
    p_commit_summary: outcome.commitSummary ?? null,
    p_files: outcome.files ?? null,
    p_message: outcome.message ?? null,
  });
  if (error) throw new Error(error.message);
}

/**
 * The cloud project a server deploys to, matched by panel + server. If the
 * team has no project linked to that server yet, one is created so the deploy
 * always has a home for its history.
 */
export async function findOrCreateProjectForServer(
  teamId: string,
  panelId: string | null,
  serverIdentifier: string,
  name: string,
): Promise<CloudProject> {
  const projects = await listProjects(teamId);
  const existing = projects.find(
    (p) => p.panel_id === panelId && p.server_identifier === serverIdentifier,
  );
  if (existing) return existing;
  return createProject(teamId, {
    name,
    panel_id: panelId,
    server_identifier: serverIdentifier,
  });
}
