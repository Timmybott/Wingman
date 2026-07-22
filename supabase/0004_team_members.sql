-- Feather cloud — team membership: invite and remove members.
--
-- Run this ONCE in the Supabase SQL editor after 0001–0003
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Members are added by email. Because a plain user cannot read auth.users,
-- both functions run SECURITY DEFINER (as the function owner) and enforce
-- their own permission checks: only a team admin/owner may add or remove
-- members, and the team owner can never be removed.

-- Add an existing Feather account to the team by email address.
create or replace function public.invite_member(p_team uuid, p_email text)
returns uuid language plpgsql security definer set search_path = public as $$
declare uid uuid;
begin
  if not public.is_team_admin(p_team) then
    raise exception 'only team admins can add members';
  end if;
  select id into uid from auth.users
    where lower(email) = lower(btrim(p_email)) limit 1;
  if uid is null then
    raise exception 'no Feather account found for %', p_email;
  end if;
  insert into public.team_members (team_id, user_id, role)
  values (p_team, uid, 'member')
  on conflict (team_id, user_id) do nothing;
  return uid;
end; $$;

grant execute on function public.invite_member(uuid, text) to authenticated;

-- Remove a member. Admins only; the owner is protected.
create or replace function public.remove_member(p_team uuid, p_user uuid)
returns void language plpgsql security definer set search_path = public as $$
begin
  if not public.is_team_admin(p_team) then
    raise exception 'only team admins can remove members';
  end if;
  if exists (select 1 from public.teams where id = p_team and owner_id = p_user) then
    raise exception 'the team owner cannot be removed';
  end if;
  delete from public.team_members where team_id = p_team and user_id = p_user;
end; $$;

grant execute on function public.remove_member(uuid, uuid) to authenticated;
