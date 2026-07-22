-- Feather cloud — project deletion tombstones.
--
-- Run this ONCE in the Supabase SQL editor after 0001–0006
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Two ways to delete a project:
--   * "Remove from Feather" just deletes the cloud project (client-side, via
--     the normal delete) and unlinks it on that device — local files are kept.
--   * "Delete everywhere" records a tombstone here and deletes the cloud
--     project. Every teammate's Feather sees the tombstone on next load and
--     deletes its own local copy of the folder. The tombstone outlives the
--     project row (it has no FK to projects) so late-syncing devices still act
--     on it.

create table if not exists public.project_deletions (
  project_id   uuid primary key,
  team_id      uuid not null references public.teams(id) on delete cascade,
  requested_by uuid references public.profiles(id),
  created_at   timestamptz not null default now()
);

alter table public.project_deletions enable row level security;

drop policy if exists project_deletions_select on public.project_deletions;
create policy project_deletions_select on public.project_deletions
  for select using (public.is_team_member(team_id));

-- Tombstone a project and delete it (cascades its deploys/issues). Membership
-- is checked; team_id is captured before the project row disappears.
create or replace function public.request_project_deletion(p_project uuid)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then
    return; -- already gone
  end if;
  if not public.is_team_member(tid) then
    raise exception 'not a member of this team';
  end if;
  insert into public.project_deletions (project_id, team_id, requested_by)
  values (p_project, tid, auth.uid())
  on conflict (project_id) do nothing;
  delete from public.projects where id = p_project;
end; $$;

grant execute on function public.request_project_deletion(uuid) to authenticated;
