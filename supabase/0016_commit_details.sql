-- Feather cloud — commit name + description, and removing the newest commit (M41).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0015
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- A commit now carries an optional Markdown `description` alongside its
-- `message` (the name/title). The newest commit of a still-pending Deploy can
-- be removed (LIFO) — later commits build on earlier ones, so only the top of
-- the stack may be popped.

alter table public.commits add column if not exists description text;

-- Recreate create_commit with a description parameter (drop the old 3-arg form
-- so a 3-arg call resolves unambiguously to the new one via its default).
drop function if exists public.create_commit(uuid, text, integer);
create or replace function public.create_commit(
  p_project uuid,
  p_message text,
  p_files integer,
  p_description text default null
)
returns public.commits
language plpgsql security definer set search_path = public as $$
declare c public.commits; b public.deploy_bundles;
begin
  if coalesce(btrim(p_message), '') = '' then
    raise exception 'commit name is required';
  end if;
  b := public.current_bundle(p_project);
  insert into public.commits (project_id, team_id, bundle_id, author_id, message, description, files_count)
  values (p_project, b.team_id, b.id, auth.uid(), btrim(p_message),
          nullif(btrim(coalesce(p_description, '')), ''), p_files)
  returning * into c;
  return c;
end; $$;

grant execute on function public.create_commit(uuid, text, integer, text) to authenticated;

-- Remove the newest commit of a pending Deploy (LIFO). Team members only.
create or replace function public.delete_commit(p_commit uuid)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid; bid uuid; bstatus text; newest uuid;
begin
  select team_id, bundle_id into tid, bid from public.commits where id = p_commit;
  if tid is null then raise exception 'commit not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  select status into bstatus from public.deploy_bundles where id = bid;
  if bstatus is distinct from 'pending' then
    raise exception 'this commit has already been deployed';
  end if;
  select id into newest from public.commits where bundle_id = bid order by created_at desc limit 1;
  if newest is distinct from p_commit then
    raise exception 'only the newest commit in the current Deploy can be removed';
  end if;
  delete from public.commits where id = p_commit;
end; $$;

grant execute on function public.delete_commit(uuid) to authenticated;
