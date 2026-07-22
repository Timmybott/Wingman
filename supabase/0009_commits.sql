-- Feather cloud — cloud commits and bundled deploys (M22).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0008
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- The DATABASE holds only metadata: which commits exist, who authored them,
-- which deploy bundle they belong to, and whether that bundle has shipped to
-- the server. The FILES themselves (each commit's snapshot) live on Feather's
-- storage backend and are reached only through the `feather-storage` Edge
-- Function — raw file bytes are never stored in Postgres.
--
-- Model: a member works locally, commits a change set (buffered as e.g.
-- "Commit v2.4.0"), and every member's commits accumulate into the project's
-- single **pending** deploy bundle — the "current Deploy". Pressing Deploy
-- *releases* that bundle: its commits ship to the server and a fresh pending
-- bundle opens for the next round.

-- ---------------------------------------------------------------------------
-- 1. Deploy bundles — the unit that ships to the server
-- ---------------------------------------------------------------------------
create table if not exists public.deploy_bundles (
  id          uuid primary key default gen_random_uuid(),
  project_id  uuid not null references public.projects(id) on delete cascade,
  team_id     uuid not null references public.teams(id) on delete cascade,
  status      text not null default 'pending'
                check (status in ('pending', 'released', 'failed')),
  created_at  timestamptz not null default now(),
  released_at timestamptz,
  released_by uuid references public.profiles(id),
  files_count integer,
  message     text
);

-- At most one pending bundle per project (the current Deploy).
create unique index if not exists deploy_bundles_one_pending
  on public.deploy_bundles (project_id) where status = 'pending';

create index if not exists deploy_bundles_project_idx
  on public.deploy_bundles (project_id, created_at desc);

-- ---------------------------------------------------------------------------
-- 2. Commits — a member's buffered change set, part of a bundle
-- ---------------------------------------------------------------------------
-- The snapshot lives on the storage backend at
--   data/<team_id>/<project_id>/commits/<commit_id>.zip
-- written through the Edge Function. `stored` flips true once that upload
-- succeeds.
create table if not exists public.commits (
  id          uuid primary key default gen_random_uuid(),
  project_id  uuid not null references public.projects(id) on delete cascade,
  team_id     uuid not null references public.teams(id) on delete cascade,
  bundle_id   uuid references public.deploy_bundles(id) on delete set null,
  author_id   uuid references public.profiles(id),
  message     text not null,
  files_count integer,
  stored      boolean not null default false,
  created_at  timestamptz not null default now()
);

create index if not exists commits_project_idx on public.commits (project_id, created_at desc);
create index if not exists commits_bundle_idx  on public.commits (bundle_id);

-- ---------------------------------------------------------------------------
-- 3. Row-Level Security — team members read; writes go through the RPCs below
-- ---------------------------------------------------------------------------
alter table public.deploy_bundles enable row level security;
alter table public.commits        enable row level security;

drop policy if exists bundles_select on public.deploy_bundles;
create policy bundles_select on public.deploy_bundles for select
  using (public.is_team_member(team_id));

drop policy if exists commits_select on public.commits;
create policy commits_select on public.commits for select
  using (public.is_team_member(team_id));

-- ---------------------------------------------------------------------------
-- 4. RPCs (SECURITY DEFINER, each enforces team membership)
-- ---------------------------------------------------------------------------

-- The project's current pending bundle, creating one if needed.
create or replace function public.current_bundle(p_project uuid)
returns public.deploy_bundles
language plpgsql security definer set search_path = public as $$
declare b public.deploy_bundles; tid uuid;
begin
  select team_id into tid from public.projects where id = p_project;
  if tid is null then raise exception 'project not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  select * into b from public.deploy_bundles
    where project_id = p_project and status = 'pending' limit 1;
  if b.id is null then
    insert into public.deploy_bundles (project_id, team_id)
      values (p_project, tid) returning * into b;
  end if;
  return b;
end; $$;

-- Create a commit in the project's current pending bundle. The client then
-- uploads the snapshot to the storage backend and calls mark_commit_stored.
create or replace function public.create_commit(p_project uuid, p_message text, p_files integer)
returns public.commits
language plpgsql security definer set search_path = public as $$
declare c public.commits; b public.deploy_bundles;
begin
  if coalesce(btrim(p_message), '') = '' then
    raise exception 'commit message is required';
  end if;
  b := public.current_bundle(p_project); -- checks membership / creates bundle
  insert into public.commits (project_id, team_id, bundle_id, author_id, message, files_count)
  values (p_project, b.team_id, b.id, auth.uid(), btrim(p_message), p_files)
  returning * into c;
  return c;
end; $$;

-- Flip a commit's `stored` flag after its snapshot upload succeeds.
create or replace function public.mark_commit_stored(p_commit uuid)
returns void language plpgsql security definer set search_path = public as $$
declare tid uuid;
begin
  select team_id into tid from public.commits where id = p_commit;
  if tid is null then raise exception 'commit not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  update public.commits set stored = true where id = p_commit;
end; $$;

-- Release the current bundle: mark it (and it stays linked to its commits)
-- released, and open a fresh pending bundle. Called after the client has
-- shipped the files to the server.
create or replace function public.release_bundle(p_project uuid, p_files integer, p_message text)
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
        files_count = p_files, message = nullif(btrim(coalesce(p_message, '')), '')
    where id = b.id
    returning * into b;
  insert into public.deploy_bundles (project_id, team_id) values (p_project, tid);
  return b;
end; $$;

grant execute on function public.current_bundle(uuid)                     to authenticated;
grant execute on function public.create_commit(uuid, text, integer)       to authenticated;
grant execute on function public.mark_commit_stored(uuid)                 to authenticated;
grant execute on function public.release_bundle(uuid, integer, text)      to authenticated;
