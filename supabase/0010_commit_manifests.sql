-- Feather cloud — commit manifests for cheap diffs (M22e).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0009
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Each commit and each released bundle now carries a lightweight manifest
-- (path → content hash) so Feather can show a "local vs server" diff without
-- downloading any archive: the current server state is the newest released
-- bundle's deployed manifest, and a commit's own manifest lets one commit be
-- diffed against another.

alter table public.commits        add column if not exists manifest          jsonb;
alter table public.deploy_bundles add column if not exists deployed_manifest jsonb;

-- Finalize a commit after its snapshot upload succeeds: record the file count
-- and manifest and flip `stored`. Replaces the plain mark_commit_stored call.
create or replace function public.finalize_commit(p_commit uuid, p_files integer, p_manifest jsonb)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid;
begin
  select team_id into tid from public.commits where id = p_commit;
  if tid is null then raise exception 'commit not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  update public.commits
    set stored = true, files_count = p_files, manifest = p_manifest
    where id = p_commit;
end; $$;

grant execute on function public.finalize_commit(uuid, integer, jsonb) to authenticated;

-- Release the current bundle, also recording the manifest that was deployed
-- (the new server state). Replaces the 0009 three-argument version.
drop function if exists public.release_bundle(uuid, integer, text);

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
  insert into public.deploy_bundles (project_id, team_id) values (p_project, tid);
  return b;
end; $$;

grant execute on function public.release_bundle(uuid, integer, text, jsonb) to authenticated;

-- The current server-state manifest for a project: the newest released
-- bundle's deployed manifest, or an empty object if it was never deployed.
create or replace function public.server_manifest(p_project uuid)
returns jsonb language plpgsql security definer stable set search_path = public as $$
declare tid uuid; m jsonb;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  select deployed_manifest into m from public.deploy_bundles
    where project_id = p_project and status = 'released' and deployed_manifest is not null
    order by released_at desc limit 1;
  return coalesce(m, '{}'::jsonb);
end; $$;

grant execute on function public.server_manifest(uuid) to authenticated;
