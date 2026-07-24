-- Feather cloud — a deploy carries a name and description (M47).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0017
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- A released Deploy already had a `message` (its name); this adds an optional
-- Markdown `description`, and recreates release_bundle so the deploying member
-- can pass both. Everything else about release_bundle is unchanged.

alter table public.deploy_bundles add column if not exists description text;

create or replace function public.release_bundle(
  p_project     uuid,
  p_files       integer,
  p_message     text,
  p_manifest    jsonb default null,
  p_description text default null
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
        description = nullif(btrim(coalesce(p_description, '')), ''),
        deployed_manifest = p_manifest
    where id = b.id
    returning * into b;
  update public.projects set server_manifest = p_manifest where id = p_project;
  insert into public.deploy_bundles (project_id, team_id) values (p_project, tid);
  return b;
end; $$;

grant execute on function public.release_bundle(uuid, integer, text, jsonb, text) to authenticated;
