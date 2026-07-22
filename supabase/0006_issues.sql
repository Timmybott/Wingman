-- Feather cloud — issues and comments per project.
--
-- Run this ONCE in the Supabase SQL editor after 0001–0005
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Each project gets a GitHub-style issue tracker. Issues are numbered per
-- project (#1, #2, …); everyone on the team can open, comment on, close and
-- reopen them.

create table if not exists public.issues (
  id         uuid primary key default gen_random_uuid(),
  project_id uuid not null references public.projects(id) on delete cascade,
  team_id    uuid not null references public.teams(id) on delete cascade,
  number     int  not null,
  title      text not null,
  body       text not null default '',
  status     text not null default 'open' check (status in ('open', 'closed')),
  created_by uuid references public.profiles(id),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  closed_at  timestamptz,
  unique (project_id, number)
);

create index if not exists issues_project_idx on public.issues (project_id, created_at desc);

create table if not exists public.issue_comments (
  id         uuid primary key default gen_random_uuid(),
  issue_id   uuid not null references public.issues(id) on delete cascade,
  team_id    uuid not null references public.teams(id) on delete cascade,
  body       text not null,
  created_by uuid references public.profiles(id),
  created_at timestamptz not null default now()
);

create index if not exists issue_comments_issue_idx on public.issue_comments (issue_id, created_at asc);

-- Keep updated_at fresh and stamp/clear closed_at as the status changes.
create or replace function public.touch_issue()
returns trigger language plpgsql as $$
begin
  new.updated_at := now();
  if new.status = 'closed' and coalesce(old.status, '') <> 'closed' then
    new.closed_at := now();
  elsif new.status = 'open' then
    new.closed_at := null;
  end if;
  return new;
end; $$;

drop trigger if exists on_issue_updated on public.issues;
create trigger on_issue_updated
  before update on public.issues
  for each row execute function public.touch_issue();

-- ---------------------------------------------------------------------------
-- Row-Level Security
-- ---------------------------------------------------------------------------
alter table public.issues         enable row level security;
alter table public.issue_comments enable row level security;

-- Reads and status/title/body edits: any team member. Inserts (which need the
-- per-project number) go through create_issue().
drop policy if exists issues_select on public.issues;
create policy issues_select on public.issues for select using (public.is_team_member(team_id));
drop policy if exists issues_update on public.issues;
create policy issues_update on public.issues for update
  using (public.is_team_member(team_id)) with check (public.is_team_member(team_id));

drop policy if exists issue_comments_select on public.issue_comments;
create policy issue_comments_select on public.issue_comments for select using (public.is_team_member(team_id));

-- ---------------------------------------------------------------------------
-- Functions
-- ---------------------------------------------------------------------------

-- Open a new issue; assigns the next per-project number.
create or replace function public.create_issue(p_project uuid, p_title text, p_body text)
returns public.issues language plpgsql security definer set search_path = public as $$
declare tid uuid; n int; row public.issues;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  if coalesce(btrim(p_title), '') = '' then raise exception 'title is required'; end if;
  select coalesce(max(number), 0) + 1 into n from public.issues where project_id = p_project;
  insert into public.issues (project_id, team_id, number, title, body, created_by)
  values (p_project, tid, n, btrim(p_title), coalesce(p_body, ''), auth.uid())
  returning * into row;
  return row;
end; $$;

grant execute on function public.create_issue(uuid, text, text) to authenticated;

-- Add a comment; team_id is derived from the issue and the author stamped.
create or replace function public.add_issue_comment(p_issue uuid, p_body text)
returns public.issue_comments language plpgsql security definer set search_path = public as $$
declare tid uuid; row public.issue_comments;
begin
  select team_id into tid from public.issues where id = p_issue;
  if tid is null then raise exception 'issue not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  if coalesce(btrim(p_body), '') = '' then raise exception 'comment cannot be empty'; end if;
  insert into public.issue_comments (issue_id, team_id, body, created_by)
  values (p_issue, tid, btrim(p_body), auth.uid())
  returning * into row;
  return row;
end; $$;

grant execute on function public.add_issue_comment(uuid, text) to authenticated;
