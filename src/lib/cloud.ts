// Typed helpers for Feather's cloud data (Supabase). More will be added per
// milestone (panels, projects, deploys, issues).

import { SUPABASE_ANON_KEY, SUPABASE_URL, supabase } from "./supabase";

export interface Team {
  id: string;
  name: string;
  owner_id: string | null;
  location: string | null;
  website: string | null;
  logo_url: string | null;
  description: string | null;
  created_at: string;
}

const TEAM_COLUMNS = "id, name, owner_id, location, website, logo_url, description, created_at";

/** Optional profile fields set when creating a team or editing it later. */
export interface TeamProfileInput {
  location?: string | null;
  website?: string | null;
  logo_url?: string | null;
  description?: string | null;
}

export async function listTeams(): Promise<Team[]> {
  const { data, error } = await supabase
    .from("teams")
    .select(TEAM_COLUMNS)
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return (data ?? []) as Team[];
}

export async function getTeam(teamId: string): Promise<Team> {
  const { data, error } = await supabase
    .from("teams")
    .select(TEAM_COLUMNS)
    .eq("id", teamId)
    .single();
  if (error) throw new Error(error.message);
  return data as Team;
}

export async function createTeam(name: string, profile: TeamProfileInput = {}): Promise<Team> {
  // Inserts server-side via a SECURITY DEFINER function (supabase/0002, 0008),
  // which stamps owner_id from auth.uid() and is not subject to the teams
  // INSERT policy — the reliable way to create a team.
  const { data, error } = await supabase.rpc("create_team", {
    p_name: name.trim(),
    p_location: profile.location ?? null,
    p_website: profile.website ?? null,
    p_logo_url: profile.logo_url ?? null,
    p_description: profile.description ?? null,
  });
  if (error) throw new Error(error.message);
  if (!data) throw new Error("team was not created");
  return (Array.isArray(data) ? data[0] : data) as Team;
}

/**
 * Edit a team's profile. Only the owner may do this — the teams_update policy
 * (supabase/0008) enforces it, so a non-owner call fails at the database.
 */
export async function updateTeam(
  teamId: string,
  fields: { name?: string } & TeamProfileInput,
): Promise<Team> {
  const patch: Record<string, string | null> = {};
  if (fields.name !== undefined) patch.name = fields.name.trim();
  if (fields.location !== undefined) patch.location = fields.location;
  if (fields.website !== undefined) patch.website = fields.website;
  if (fields.logo_url !== undefined) patch.logo_url = fields.logo_url;
  if (fields.description !== undefined) patch.description = fields.description;
  const { data, error } = await supabase
    .from("teams")
    .update(patch)
    .eq("id", teamId)
    .select(TEAM_COLUMNS)
    .single();
  if (error) throw new Error(error.message);
  return data as Team;
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

/**
 * Delete a project everywhere: records a tombstone (so every teammate's
 * Feather deletes its local copy on next load) and deletes the cloud project.
 */
export async function requestProjectDeletion(id: string): Promise<void> {
  const { error } = await supabase.rpc("request_project_deletion", { p_project: id });
  if (error) throw new Error(error.message);
}

/** Project ids the team has tombstoned for "delete everywhere". */
export async function listProjectDeletions(teamId: string): Promise<string[]> {
  const { data, error } = await supabase
    .from("project_deletions")
    .select("project_id")
    .eq("team_id", teamId);
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => row.project_id);
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

/**
 * Grant or revoke admin rights — owner only (enforced by set_member_role in
 * supabase/0008). Pass "admin" to promote, "member" to demote.
 */
export async function setMemberRole(
  teamId: string,
  userId: string,
  role: "admin" | "member",
): Promise<void> {
  const { error } = await supabase.rpc("set_member_role", {
    p_team: teamId,
    p_user: userId,
    p_role: role,
  });
  if (error) throw new Error(error.message);
}

// --- User profiles ---------------------------------------------------------

export interface UserProfile {
  id: string;
  username: string | null;
  display_name: string | null;
  location: string | null;
  website: string | null;
  avatar_url: string | null;
  bio: string | null;
  created_at: string;
}

const PROFILE_COLUMNS =
  "id, username, display_name, location, website, avatar_url, bio, created_at";

/** Any user's public profile (profiles are readable by everyone). */
export async function getProfile(userId: string): Promise<UserProfile> {
  const { data, error } = await supabase
    .from("profiles")
    .select(PROFILE_COLUMNS)
    .eq("id", userId)
    .single();
  if (error) throw new Error(error.message);
  return data as UserProfile;
}

/** Edit the signed-in user's own profile (profiles_update_own policy). */
export async function updateMyProfile(fields: {
  display_name?: string | null;
  location?: string | null;
  website?: string | null;
  avatar_url?: string | null;
  bio?: string | null;
}): Promise<UserProfile> {
  const { data: userData } = await supabase.auth.getUser();
  const uid = userData.user?.id;
  if (!uid) throw new Error("not signed in");
  const { data, error } = await supabase
    .from("profiles")
    .update(fields)
    .eq("id", uid)
    .select(PROFILE_COLUMNS)
    .single();
  if (error) throw new Error(error.message);
  return data as UserProfile;
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

// --- Issues ----------------------------------------------------------------

export type IssueStatus = "open" | "closed";

export interface Issue {
  id: string;
  number: number;
  title: string;
  body: string;
  status: IssueStatus;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  created_by: string | null;
  author_name: string | null;
  comment_count: number;
}

export interface IssueComment {
  id: string;
  body: string;
  created_at: string;
  created_by: string | null;
  author_name: string | null;
}

function profileName(raw: unknown): string | null {
  const profile = (Array.isArray(raw) ? raw[0] : raw) as
    | { display_name: string | null; username: string | null }
    | null
    | undefined;
  return profile?.display_name?.trim() || profile?.username || null;
}

export async function listIssues(projectId: string): Promise<Issue[]> {
  const { data, error } = await supabase
    .from("issues")
    .select(
      "id, number, title, body, status, created_at, updated_at, closed_at, created_by, profiles(display_name, username), issue_comments(count)",
    )
    .eq("project_id", projectId)
    .order("created_at", { ascending: false });
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => {
    const counts = (row as { issue_comments?: { count: number }[] }).issue_comments;
    return {
      id: row.id,
      number: row.number,
      title: row.title,
      body: row.body,
      status: row.status,
      created_at: row.created_at,
      updated_at: row.updated_at,
      closed_at: row.closed_at,
      created_by: row.created_by,
      author_name: profileName((row as { profiles?: unknown }).profiles),
      comment_count: counts?.[0]?.count ?? 0,
    };
  });
}

export async function createIssue(
  projectId: string,
  title: string,
  body: string,
): Promise<void> {
  const { error } = await supabase.rpc("create_issue", {
    p_project: projectId,
    p_title: title,
    p_body: body,
  });
  if (error) throw new Error(error.message);
}

export async function setIssueStatus(id: string, status: IssueStatus): Promise<void> {
  const { error } = await supabase.from("issues").update({ status }).eq("id", id);
  if (error) throw new Error(error.message);
}

export async function listComments(issueId: string): Promise<IssueComment[]> {
  const { data, error } = await supabase
    .from("issue_comments")
    .select("id, body, created_at, created_by, profiles(display_name, username)")
    .eq("issue_id", issueId)
    .order("created_at", { ascending: true });
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => ({
    id: row.id,
    body: row.body,
    created_at: row.created_at,
    created_by: row.created_by,
    author_name: profileName((row as { profiles?: unknown }).profiles),
  }));
}

export async function addComment(issueId: string, body: string): Promise<void> {
  const { error } = await supabase.rpc("add_issue_comment", {
    p_issue: issueId,
    p_body: body,
  });
  if (error) throw new Error(error.message);
}

// --- Cloud commits & deploy bundles (M22) ----------------------------------
//
// A member commits their local change set (buffered as e.g. "Commit v2.4.0").
// Every member's commits accumulate into the project's single pending deploy
// bundle — the "current Deploy". Releasing that bundle ships its commits to the
// server and opens a fresh one. The database holds only metadata; the file
// snapshots live on the storage backend, reached through the feather-storage
// Edge Function (see putSnapshot/getSnapshot below).

export type BundleStatus = "pending" | "released" | "failed";

export interface DeployBundle {
  id: string;
  project_id: string;
  team_id: string;
  status: BundleStatus;
  created_at: string;
  released_at: string | null;
  released_by: string | null;
  files_count: number | null;
  message: string | null;
}

export interface CloudCommit {
  id: string;
  project_id: string;
  bundle_id: string | null;
  author_id: string | null;
  message: string;
  files_count: number | null;
  stored: boolean;
  created_at: string;
  author_name: string | null;
}

function bundleFrom(data: unknown): DeployBundle {
  return (Array.isArray(data) ? data[0] : data) as DeployBundle;
}

/** The project's current pending bundle, creating one if none exists. */
export async function currentBundle(projectId: string): Promise<DeployBundle> {
  const { data, error } = await supabase.rpc("current_bundle", { p_project: projectId });
  if (error) throw new Error(error.message);
  return bundleFrom(data);
}

/** Create a commit in the project's current pending bundle. */
export async function createCommit(
  projectId: string,
  message: string,
  files: number | null,
): Promise<CloudCommit> {
  const { data, error } = await supabase.rpc("create_commit", {
    p_project: projectId,
    p_message: message,
    p_files: files,
  });
  if (error) throw new Error(error.message);
  return (Array.isArray(data) ? data[0] : data) as CloudCommit;
}

/** Flag a commit's snapshot as uploaded (after putSnapshot succeeds). */
export async function markCommitStored(commitId: string): Promise<void> {
  const { error } = await supabase.rpc("mark_commit_stored", { p_commit: commitId });
  if (error) throw new Error(error.message);
}

/** Release the current bundle: ship it and open a fresh one. */
export async function releaseBundle(
  projectId: string,
  files: number | null,
  message: string | null,
): Promise<DeployBundle> {
  const { data, error } = await supabase.rpc("release_bundle", {
    p_project: projectId,
    p_files: files,
    p_message: message,
  });
  if (error) throw new Error(error.message);
  return bundleFrom(data);
}

/** All bundles of a project, newest first. */
export async function listBundles(projectId: string): Promise<DeployBundle[]> {
  const { data, error } = await supabase
    .from("deploy_bundles")
    .select(
      "id, project_id, team_id, status, created_at, released_at, released_by, files_count, message",
    )
    .eq("project_id", projectId)
    .order("created_at", { ascending: false });
  if (error) throw new Error(error.message);
  return (data ?? []) as DeployBundle[];
}

/** Commits of a project (optionally limited to one bundle), newest first. */
export async function listCommits(projectId: string, bundleId?: string): Promise<CloudCommit[]> {
  let query = supabase
    .from("commits")
    .select(
      "id, project_id, bundle_id, author_id, message, files_count, stored, created_at, profiles(display_name, username)",
    )
    .eq("project_id", projectId)
    .order("created_at", { ascending: false });
  if (bundleId) query = query.eq("bundle_id", bundleId);
  const { data, error } = await query;
  if (error) throw new Error(error.message);
  return (data ?? []).map((row) => ({
    id: row.id,
    project_id: row.project_id,
    bundle_id: row.bundle_id,
    author_id: row.author_id,
    message: row.message,
    files_count: row.files_count,
    stored: row.stored,
    created_at: row.created_at,
    author_name: profileName((row as { profiles?: unknown }).profiles),
  }));
}

// --- Storage backend (feather-storage Edge Function) -----------------------
//
// Commit/rollback snapshots are stored as files on Feather's storage server,
// reached only through the Edge Function so its API key stays server-side. The
// function derives the path from the ids; we only pass ids and the bytes.

export type SnapshotKind = "commit" | "rollback";

const STORAGE_ENDPOINT = `${SUPABASE_URL}/functions/v1/feather-storage`;

async function storageHeaders(): Promise<Record<string, string>> {
  const { data } = await supabase.auth.getSession();
  const token = data.session?.access_token;
  if (!token) throw new Error("not signed in");
  return { Authorization: `Bearer ${token}`, apikey: SUPABASE_ANON_KEY };
}

function storageUrl(
  action: string,
  projectId: string,
  commitId: string | null,
  kind: SnapshotKind,
): string {
  const params = new URLSearchParams({ action, project_id: projectId, kind });
  if (commitId) params.set("commit_id", commitId);
  return `${STORAGE_ENDPOINT}?${params.toString()}`;
}

/** Upload a commit/rollback snapshot (a zip) to the storage backend. */
export async function putSnapshot(
  projectId: string,
  commitId: string,
  kind: SnapshotKind,
  bytes: ArrayBuffer | Uint8Array,
): Promise<void> {
  const res = await fetch(storageUrl("put", projectId, commitId, kind), {
    method: "POST",
    headers: { ...(await storageHeaders()), "content-type": "application/octet-stream" },
    // Both ArrayBuffer and Uint8Array are valid fetch bodies at runtime; the
    // cast sidesteps the DOM lib's ArrayBufferLike/SharedArrayBuffer generics.
    body: bytes as BodyInit,
  });
  if (!res.ok) throw new Error(`snapshot upload failed (${res.status})`);
}

/** Download a snapshot (a zip) from the storage backend. */
export async function getSnapshot(
  projectId: string,
  commitId: string,
  kind: SnapshotKind,
): Promise<ArrayBuffer> {
  const res = await fetch(storageUrl("get", projectId, commitId, kind), {
    headers: await storageHeaders(),
  });
  if (!res.ok) throw new Error(`snapshot download failed (${res.status})`);
  return await res.arrayBuffer();
}

/** Delete a snapshot from the storage backend. */
export async function deleteSnapshot(
  projectId: string,
  commitId: string,
  kind: SnapshotKind,
): Promise<void> {
  const res = await fetch(storageUrl("delete", projectId, commitId, kind), {
    method: "POST",
    headers: await storageHeaders(),
  });
  if (!res.ok) throw new Error(`snapshot delete failed (${res.status})`);
}

/**
 * Whether the storage backend is configured. The function returns 503 until
 * its key is set, so anything else (even a 400 for the empty probe) means it's
 * live. Best-effort: any network error reports unavailable.
 */
export async function storageAvailable(): Promise<boolean> {
  try {
    const res = await fetch(`${STORAGE_ENDPOINT}?action=ping`, {
      headers: await storageHeaders(),
    });
    return res.status !== 503;
  } catch {
    return false;
  }
}
