-- Feather cloud — a reliable server-state baseline for diffs (M25).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0012
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- The Deploy tab diffs the local folder against "what's on the server". That
-- baseline used to come only from the last *released* bundle, so a freshly
-- imported project (no cloud deploy yet) had an empty baseline and every file
-- showed as newly added. The baseline now lives on the project itself and is
-- set both when the server's files are imported and when a deploy is released.

alter table public.projects add column if not exists server_manifest jsonb;

-- Record the project's known server-state manifest. Called by the app right
-- after it imports the server's files into a local folder.
create or replace function public.set_server_manifest(p_project uuid, p_manifest jsonb)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  update public.projects set server_manifest = p_manifest where id = p_project;
end; $$;

grant execute on function public.set_server_manifest(uuid, jsonb) to authenticated;

-- server_manifest() now reads the project baseline (falling back to the latest
-- released bundle for projects deployed before this migration, then to {}).
create or replace function public.server_manifest(p_project uuid)
returns jsonb language plpgsql security definer stable set search_path = public as $$
declare tid uuid; m jsonb;
begin
  select team_id, server_manifest into tid, m from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  if m is not null then return m; end if;
  select deployed_manifest into m from public.deploy_bundles
    where project_id = p_project and status = 'released' and deployed_manifest is not null
    order by released_at desc limit 1;
  return coalesce(m, '{}'::jsonb);
end; $$;

grant execute on function public.server_manifest(uuid) to authenticated;

-- Releasing a bundle now also updates the project baseline, so the diff resets
-- to the freshly deployed state.
create or replace function public.release_bundle(
  p_project  uuid,
  p_files    integer,
  p_message  text,
  p_manifest jsonb default null
)
returns public.deploy_bundles
language plpgsql security definer set search_path = public as $$
declare b public.deploy_bundles; tid uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  select * into b from public.deploy_bundles
    where project_id = p_project and status = 'pending' limit 1;
  if b.id is null then raise exception 'nothing to deploy'; end if;
  update public.deploy_bundles
    set status = 'released', released_at = now(), released_by = auth.uid(),
        files_count = p_files, message = nullif(btrim(coalesce(p_message, '')), ''),
        deployed_manifest = p_manifest
    where id = b.id
    returning * into b;
  update public.projects set server_manifest = p_manifest where id = p_project;
  insert into public.deploy_bundles (project_id, team_id) values (p_project, tid);
  return b;
end; $$;

grant execute on function public.release_bundle(uuid, integer, text, jsonb) to authenticated;
