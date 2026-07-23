-- Feather cloud — authenticated read of teams, projects and their content, so
-- profile and team pages show the real, full picture (M46).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0016
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Feather is GitHub-like and its projects are open source, so any signed-in
-- user can READ teams, who's on them, projects, deploy history, commits and
-- issues. This fixes profile pages showing only the teams/projects you happen
-- to *share* with the person: a user's profile now lists all of their teams and
-- projects, and you can browse another team's project read-only.
--
-- Writes stay locked down exactly as before: creating/editing/deleting teams,
-- projects, commits, deploys and issues still go through the same
-- membership/owner checks (their INSERT/UPDATE/DELETE policies and the
-- SECURITY DEFINER RPCs are untouched). Panels stay members-only — they hold
-- the encrypted API keys — and the plaintext key is still only reachable
-- through panel_api_key() after a membership check.
--
-- Note: the client's team picker (listTeams) no longer relies on the teams
-- SELECT policy to scope results — it filters by the signed-in user's own
-- membership rows — so opening up reads here does not flood the picker.

-- Teams and who's on them.
drop policy if exists teams_select on public.teams;
create policy teams_select on public.teams
  for select using (auth.uid() is not null);

drop policy if exists members_select on public.team_members;
create policy members_select on public.team_members
  for select using (auth.uid() is not null);

-- Projects and everything a project page reads.
drop policy if exists projects_select on public.projects;
create policy projects_select on public.projects
  for select using (auth.uid() is not null);

drop policy if exists deploys_select on public.deploys;
create policy deploys_select on public.deploys
  for select using (auth.uid() is not null);

drop policy if exists bundles_select on public.deploy_bundles;
create policy bundles_select on public.deploy_bundles
  for select using (auth.uid() is not null);

drop policy if exists commits_select on public.commits;
create policy commits_select on public.commits
  for select using (auth.uid() is not null);

drop policy if exists issues_select on public.issues;
create policy issues_select on public.issues
  for select using (auth.uid() is not null);

drop policy if exists issue_comments_select on public.issue_comments;
create policy issue_comments_select on public.issue_comments
  for select using (auth.uid() is not null);
