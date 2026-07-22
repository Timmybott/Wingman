-- Feather cloud — profile pages for users and teams, plus owner-managed roles.
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0007
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Adds self-customizable profile fields (a few structured fields + a Markdown
-- README) to both user accounts and teams, tightens team editing to the owner,
-- and lets the owner grant/revoke admin rights.

-- ---------------------------------------------------------------------------
-- 1. User profile fields — self-editable via the existing profiles_update_own
--    policy, readable by everyone via profiles_read.
-- ---------------------------------------------------------------------------
alter table public.profiles add column if not exists location   text;
alter table public.profiles add column if not exists website    text;
alter table public.profiles add column if not exists avatar_url text;
alter table public.profiles add column if not exists bio        text; -- Markdown

-- ---------------------------------------------------------------------------
-- 2. Team profile fields
-- ---------------------------------------------------------------------------
alter table public.teams add column if not exists location    text;
alter table public.teams add column if not exists website     text;
alter table public.teams add column if not exists logo_url    text;
alter table public.teams add column if not exists description text; -- Markdown

-- ---------------------------------------------------------------------------
-- 3. Owner check helper (SECURITY DEFINER, like is_team_member/is_team_admin)
-- ---------------------------------------------------------------------------
create or replace function public.is_team_owner(p_team uuid)
returns boolean language sql security definer stable set search_path = public as $$
  select exists (
    select 1 from public.teams where id = p_team and owner_id = auth.uid()
  );
$$;

grant execute on function public.is_team_owner(uuid) to authenticated;

-- ---------------------------------------------------------------------------
-- 4. The team profile is editable by the OWNER only (it was admin). Admins
--    keep member management, which runs through the SECURITY DEFINER RPCs
--    below and so is unaffected by this policy.
-- ---------------------------------------------------------------------------
drop policy if exists teams_update on public.teams;
create policy teams_update on public.teams for update
  using (owner_id = auth.uid())
  with check (owner_id = auth.uid());

-- Direct writes to team_members are owner-only too, so an admin cannot escalate
-- roles straight through the table API. invite_member/remove_member/
-- set_member_role are SECURITY DEFINER and bypass RLS, so admins keep the
-- ability to add and remove members. members_select still lets members read.
drop policy if exists members_manage on public.team_members;
create policy members_manage on public.team_members for all
  using (public.is_team_owner(team_id))
  with check (public.is_team_owner(team_id));

-- ---------------------------------------------------------------------------
-- 5. Grant or revoke admin rights — owner only. The owner's own role is fixed,
--    and only 'admin' or 'member' can be assigned.
-- ---------------------------------------------------------------------------
create or replace function public.set_member_role(p_team uuid, p_user uuid, p_role text)
returns void language plpgsql security definer set search_path = public as $$
begin
  if not public.is_team_owner(p_team) then
    raise exception 'only the team owner can change roles';
  end if;
  if p_role not in ('admin', 'member') then
    raise exception 'role must be admin or member';
  end if;
  if exists (select 1 from public.teams where id = p_team and owner_id = p_user) then
    raise exception 'the owner role cannot be changed';
  end if;
  update public.team_members set role = p_role
    where team_id = p_team and user_id = p_user;
  if not found then
    raise exception 'that user is not a member of this team';
  end if;
end; $$;

grant execute on function public.set_member_role(uuid, uuid, text) to authenticated;

-- ---------------------------------------------------------------------------
-- 6. Team creation accepts the optional profile fields. Replaces the 1-arg
--    create_team from 0002; PostgREST resolves calls that pass only p_name via
--    the defaults.
-- ---------------------------------------------------------------------------
drop function if exists public.create_team(text);

create or replace function public.create_team(
  p_name        text,
  p_location    text default null,
  p_website     text default null,
  p_logo_url    text default null,
  p_description text default null
) returns public.teams
language plpgsql security definer set search_path = public as $$
declare t public.teams;
begin
  if auth.uid() is null then
    raise exception 'not signed in';
  end if;
  if coalesce(btrim(p_name), '') = '' then
    raise exception 'team name is required';
  end if;
  insert into public.teams (name, owner_id, location, website, logo_url, description)
  values (
    btrim(p_name),
    auth.uid(),
    nullif(btrim(coalesce(p_location, '')), ''),
    nullif(btrim(coalesce(p_website, '')), ''),
    nullif(btrim(coalesce(p_logo_url, '')), ''),
    nullif(btrim(coalesce(p_description, '')), '')
  )
  returning * into t;
  return t;
end; $$;

grant execute on function public.create_team(text, text, text, text, text) to authenticated;
