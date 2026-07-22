-- Feather cloud — deploy history.
--
-- Run this ONCE in the Supabase SQL editor after 0001–0004
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Every deploy or rollback a teammate runs is recorded here so the project's
-- Deploys tab shows the full history: who, when, which commit, how many files,
-- and whether it succeeded. The deploy engine itself still runs locally on
-- each machine; this table only stores the shared record of what happened.

create table if not exists public.deploys (
  id             uuid primary key default gen_random_uuid(),
  project_id     uuid not null references public.projects(id) on delete cascade,
  team_id        uuid not null references public.teams(id) on delete cascade,
  user_id        uuid references public.profiles(id),
  kind           text not null default 'deploy' check (kind in ('deploy', 'rollback', 'sync')),
  status         text not null check (status in ('success', 'failed')),
  commit         text,
  commit_summary text,
  files_count    int,
  message        text,
  created_at     timestamptz not null default now()
);

create index if not exists deploys_project_idx on public.deploys (project_id, created_at desc);

alter table public.deploys enable row level security;

drop policy if exists deploys_select on public.deploys;
create policy deploys_select on public.deploys for select using (public.is_team_member(team_id));

-- Record a deploy. team_id and user_id are derived server-side so the client
-- cannot forge them; membership is checked before inserting.
create or replace function public.record_deploy(
  p_project uuid, p_kind text, p_status text, p_commit text,
  p_commit_summary text, p_files int, p_message text
) returns uuid language plpgsql security definer set search_path = public as $$
declare tid uuid; new_id uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  insert into public.deploys
    (project_id, team_id, user_id, kind, status, commit, commit_summary, files_count, message)
  values
    (p_project, tid, auth.uid(), coalesce(nullif(p_kind, ''), 'deploy'), p_status,
     p_commit, p_commit_summary, p_files, p_message)
  returning id into new_id;
  return new_id;
end; $$;

grant execute on function public.record_deploy(uuid, text, text, text, text, int, text) to authenticated;
