-- Feather cloud — connect issues with deploys and commits (M23).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0010
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- A new issue is filed against the project's current Deploy (the pending
-- bundle), so a deploy's page can show the issues raised in that cycle. A
-- resolved issue can then be pinned to the specific commit that fixed it.

alter table public.issues
  add column if not exists bundle_id uuid references public.deploy_bundles(id) on delete set null;
alter table public.issues
  add column if not exists commit_id uuid references public.commits(id) on delete set null;

create index if not exists issues_bundle_idx on public.issues (bundle_id);

-- Recreate create_issue so a new issue is linked to the project's current
-- pending deploy bundle (creating one if the project has none yet).
create or replace function public.create_issue(p_project uuid, p_title text, p_body text)
returns public.issues language plpgsql security definer set search_path = public as $$
declare tid uuid; n int; row public.issues; bid uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  if coalesce(btrim(p_title), '') = '' then raise exception 'title is required'; end if;
  select id into bid from public.deploy_bundles
    where project_id = p_project and status = 'pending' limit 1;
  if bid is null then
    insert into public.deploy_bundles (project_id, team_id)
      values (p_project, tid) returning id into bid;
  end if;
  select coalesce(max(number), 0) + 1 into n from public.issues where project_id = p_project;
  insert into public.issues (project_id, team_id, number, title, body, created_by, bundle_id)
  values (p_project, tid, n, btrim(p_title), coalesce(p_body, ''), auth.uid(), bid)
  returning * into row;
  return row;
end; $$;

grant execute on function public.create_issue(uuid, text, text) to authenticated;

-- Pin (or unpin, with p_commit null) an issue to the commit that resolved it.
-- The commit must belong to the same project; the issue then moves to that
-- commit's deploy bundle so it shows under the right deploy.
create or replace function public.assign_issue_commit(p_issue uuid, p_commit uuid)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid; iproj uuid; cproj uuid; cbundle uuid;
begin
  select team_id, project_id into tid, iproj from public.issues where id = p_issue;
  if tid is null then raise exception 'issue not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  if p_commit is null then
    update public.issues set commit_id = null where id = p_issue;
    return;
  end if;
  select project_id, bundle_id into cproj, cbundle from public.commits where id = p_commit;
  if cproj is null then raise exception 'commit not found'; end if;
  if cproj is distinct from iproj then raise exception 'that commit is not in this project'; end if;
  update public.issues
    set commit_id = p_commit, bundle_id = coalesce(cbundle, bundle_id)
    where id = p_issue;
end; $$;

grant execute on function public.assign_issue_commit(uuid, uuid) to authenticated;
