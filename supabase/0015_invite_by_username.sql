-- Feather cloud — add members by email OR username (M40).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0014
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Recreates invite_member so the identifier can be either an email address
-- (matched against auth.users) or a username (matched against profiles).
-- Signature is unchanged, so the app keeps calling it the same way.

create or replace function public.invite_member(p_team uuid, p_email text)
returns uuid language plpgsql security definer set search_path = public as $$
declare uid uuid; ident text := btrim(p_email);
begin
  if not public.is_team_admin(p_team) then
    raise exception 'only team admins can add members';
  end if;
  if ident = '' then
    raise exception 'enter an email or username';
  end if;
  -- Match by email first, then by username.
  select id into uid from auth.users where lower(email) = lower(ident) limit 1;
  if uid is null then
    select id into uid from public.profiles where lower(username) = lower(ident) limit 1;
  end if;
  if uid is null then
    raise exception 'no Feather account found for %', ident;
  end if;
  insert into public.team_members (team_id, user_id, role)
  values (p_team, uid, 'member')
  on conflict (team_id, user_id) do nothing;
  return uid;
end; $$;

grant execute on function public.invite_member(uuid, text) to authenticated;
