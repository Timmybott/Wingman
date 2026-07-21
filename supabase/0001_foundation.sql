-- Feather cloud foundation — accounts, teams, encrypted panels, projects.
--
-- Run this ONCE in the Supabase SQL editor (Dashboard → SQL Editor → New query
-- → paste → Run). Prerequisite: create the encryption secret first, see
-- docs/CLOUD-SETUP.md step 2. Safe design notes:
--   * Every table is protected by Row-Level Security so a user only ever sees
--     the teams they belong to.
--   * Pterodactyl API keys are NEVER stored in plaintext. They are encrypted
--     at rest with a master key kept in Supabase Vault, and can only be
--     decrypted through panel_api_key(), which checks team membership first.

create extension if not exists pgcrypto;

-- ---------------------------------------------------------------------------
-- 1. Profiles (one row per auth user)
-- ---------------------------------------------------------------------------
create table if not exists public.profiles (
  id           uuid primary key references auth.users(id) on delete cascade,
  username     text unique,
  display_name text,
  created_at   timestamptz not null default now()
);

create or replace function public.handle_new_user()
returns trigger language plpgsql security definer set search_path = public as $$
begin
  insert into public.profiles (id, display_name, username)
  values (
    new.id,
    coalesce(new.raw_user_meta_data->>'display_name', split_part(new.email, '@', 1)),
    split_part(new.email, '@', 1)
  )
  on conflict (id) do nothing;
  return new;
end; $$;

drop trigger if exists on_auth_user_created on auth.users;
create trigger on_auth_user_created
  after insert on auth.users
  for each row execute function public.handle_new_user();

-- ---------------------------------------------------------------------------
-- 2. Teams and membership (the unit of collaboration)
-- ---------------------------------------------------------------------------
create table if not exists public.teams (
  id         uuid primary key default gen_random_uuid(),
  name       text not null,
  owner_id   uuid not null references public.profiles(id) on delete restrict,
  created_at timestamptz not null default now()
);

create table if not exists public.team_members (
  team_id    uuid not null references public.teams(id) on delete cascade,
  user_id    uuid not null references public.profiles(id) on delete cascade,
  role       text not null default 'member' check (role in ('owner', 'admin', 'member')),
  created_at timestamptz not null default now(),
  primary key (team_id, user_id)
);

-- Membership checks run as SECURITY DEFINER so they bypass RLS — this is what
-- prevents the classic "policy on team_members that queries team_members"
-- infinite recursion.
create or replace function public.is_team_member(p_team uuid)
returns boolean language sql security definer stable set search_path = public as $$
  select exists (
    select 1 from public.team_members
    where team_id = p_team and user_id = auth.uid()
  );
$$;

create or replace function public.is_team_admin(p_team uuid)
returns boolean language sql security definer stable set search_path = public as $$
  select exists (
    select 1 from public.team_members
    where team_id = p_team and user_id = auth.uid() and role in ('owner', 'admin')
  );
$$;

-- Adding the creator as owner-member right after a team is created.
create or replace function public.handle_new_team()
returns trigger language plpgsql security definer set search_path = public as $$
begin
  insert into public.team_members (team_id, user_id, role)
  values (new.id, new.owner_id, 'owner')
  on conflict do nothing;
  return new;
end; $$;

drop trigger if exists on_team_created on public.teams;
create trigger on_team_created
  after insert on public.teams
  for each row execute function public.handle_new_team();

-- ---------------------------------------------------------------------------
-- 3. Panels (Pterodactyl connections) — API key encrypted, shared in the team
-- ---------------------------------------------------------------------------
create table if not exists public.panels (
  id                uuid primary key default gen_random_uuid(),
  team_id           uuid not null references public.teams(id) on delete cascade,
  name              text not null,
  base_url          text not null,
  api_key_encrypted bytea not null,
  created_by        uuid references public.profiles(id),
  created_at        timestamptz not null default now()
);

-- The master key lives in Supabase Vault (see docs/CLOUD-SETUP.md step 2).
create or replace function public.feather_master_key()
returns text language sql security definer stable set search_path = public, vault as $$
  select decrypted_secret from vault.decrypted_secrets
  where name = 'feather_encryption_key' limit 1;
$$;

-- Create a panel: encrypts the key server-side; plaintext never touches a table.
create or replace function public.create_panel(
  p_team uuid, p_name text, p_base_url text, p_api_key text
) returns uuid language plpgsql security definer set search_path = public as $$
declare new_id uuid;
begin
  if not public.is_team_member(p_team) then
    raise exception 'not a member of this team';
  end if;
  insert into public.panels (team_id, name, base_url, api_key_encrypted, created_by)
  values (p_team, p_name, p_base_url,
          pgp_sym_encrypt(p_api_key, public.feather_master_key()), auth.uid())
  returning id into new_id;
  return new_id;
end; $$;

-- Decrypt a panel's key — team members only. This is the ONLY way to read it.
create or replace function public.panel_api_key(p_panel uuid)
returns text language plpgsql security definer set search_path = public as $$
declare enc bytea; tid uuid;
begin
  select api_key_encrypted, team_id into enc, tid from public.panels where id = p_panel;
  if tid is null then raise exception 'panel not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  return pgp_sym_decrypt(enc, public.feather_master_key());
end; $$;

-- ---------------------------------------------------------------------------
-- 4. Projects (metadata shared in the team; the local folder stays per-device)
-- ---------------------------------------------------------------------------
create table if not exists public.projects (
  id                uuid primary key default gen_random_uuid(),
  team_id           uuid not null references public.teams(id) on delete cascade,
  name              text not null,
  description       text default '',
  panel_id          uuid references public.panels(id) on delete set null,
  server_identifier text,
  target_dir        text default '',
  build_command     text,
  post_deploy       text default 'restart' check (post_deploy in ('restart', 'notify')),
  auto_backup       boolean default true,
  created_by        uuid references public.profiles(id),
  created_at        timestamptz not null default now()
);

-- ---------------------------------------------------------------------------
-- 5. Row-Level Security
-- ---------------------------------------------------------------------------
alter table public.profiles     enable row level security;
alter table public.teams        enable row level security;
alter table public.team_members enable row level security;
alter table public.panels       enable row level security;
alter table public.projects     enable row level security;

-- profiles: readable by everyone (to show names), self-editable
drop policy if exists profiles_read on public.profiles;
create policy profiles_read on public.profiles for select using (true);
drop policy if exists profiles_update_own on public.profiles;
create policy profiles_update_own on public.profiles for update using (id = auth.uid());

-- teams
drop policy if exists teams_select on public.teams;
create policy teams_select on public.teams for select using (public.is_team_member(id));
drop policy if exists teams_insert on public.teams;
create policy teams_insert on public.teams for insert with check (owner_id = auth.uid());
drop policy if exists teams_update on public.teams;
create policy teams_update on public.teams for update using (public.is_team_admin(id));
drop policy if exists teams_delete on public.teams;
create policy teams_delete on public.teams for delete using (owner_id = auth.uid());

-- team_members
drop policy if exists members_select on public.team_members;
create policy members_select on public.team_members for select using (public.is_team_member(team_id));
drop policy if exists members_manage on public.team_members;
create policy members_manage on public.team_members for all
  using (public.is_team_admin(team_id))
  with check (public.is_team_admin(team_id));

-- panels: members can read metadata + delete; inserts go through create_panel()
drop policy if exists panels_select on public.panels;
create policy panels_select on public.panels for select using (public.is_team_member(team_id));
drop policy if exists panels_delete on public.panels;
create policy panels_delete on public.panels for delete using (public.is_team_member(team_id));

-- projects: full access for team members
drop policy if exists projects_select on public.projects;
create policy projects_select on public.projects for select using (public.is_team_member(team_id));
drop policy if exists projects_insert on public.projects;
create policy projects_insert on public.projects for insert with check (public.is_team_member(team_id));
drop policy if exists projects_update on public.projects;
create policy projects_update on public.projects for update using (public.is_team_member(team_id));
drop policy if exists projects_delete on public.projects;
create policy projects_delete on public.projects for delete using (public.is_team_member(team_id));

-- ---------------------------------------------------------------------------
-- 6. Allow the app (authenticated role) to call the helper functions
-- ---------------------------------------------------------------------------
grant execute on function public.create_panel(uuid, text, text, text) to authenticated;
grant execute on function public.panel_api_key(uuid)                  to authenticated;
grant execute on function public.is_team_member(uuid)                 to authenticated;
grant execute on function public.is_team_admin(uuid)                  to authenticated;
