-- Feather cloud — robust team creation (fixes a broken teams INSERT path).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001_foundation.sql
-- (Dashboard → SQL Editor → New query → paste → Run). It is idempotent, so
-- re-running it is safe.
--
-- Why this exists: creating a team by inserting directly into `public.teams`
-- depends on the row-level INSERT policy evaluating `owner_id = auth.uid()`
-- exactly right. That path proved fragile in practice (a fresh account hit
-- "new row violates row-level security policy for table teams"). Panels already
-- avoid this by inserting through a SECURITY DEFINER function; teams now do the
-- same. The function runs with the definer's rights, so it is not subject to
-- the INSERT policy, and it stamps owner_id from auth.uid() server-side — the
-- client can never forge it.

-- Repair the direct INSERT policy as well, so both paths are correct.
drop policy if exists teams_insert on public.teams;
create policy teams_insert on public.teams
  for insert with check (owner_id = auth.uid());

-- Create a team and return the new row. The on_team_created trigger adds the
-- creator as an owner-member right after the insert.
create or replace function public.create_team(p_name text)
returns public.teams
language plpgsql security definer set search_path = public as $$
declare t public.teams;
begin
  if auth.uid() is null then
    raise exception 'not signed in';
  end if;
  if coalesce(btrim(p_name), '') = '' then
    raise exception 'team name is required';
  end if;
  insert into public.teams (name, owner_id)
  values (btrim(p_name), auth.uid())
  returning * into t;
  return t;
end; $$;

grant execute on function public.create_team(text) to authenticated;
