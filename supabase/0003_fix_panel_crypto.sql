-- Feather cloud — fix panel key encryption/decryption.
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001 and 0002
-- (Dashboard → SQL Editor → New query → paste → Run). It is idempotent.
--
-- Why this exists: Supabase installs the pgcrypto extension (which provides
-- pgp_sym_encrypt / pgp_sym_decrypt) into the `extensions` schema, not
-- `public`. The panel functions pinned `search_path = public`, so they could
-- not find those functions and failed with:
--   "function pgp_sym_encrypt(text, text) does not exist"
-- Adding `extensions` to the search_path resolves them regardless of which
-- schema pgcrypto lives in.

create or replace function public.create_panel(
  p_team uuid, p_name text, p_base_url text, p_api_key text
) returns uuid language plpgsql security definer set search_path = public, extensions as $$
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

create or replace function public.panel_api_key(p_panel uuid)
returns text language plpgsql security definer set search_path = public, extensions as $$
declare enc bytea; tid uuid;
begin
  select api_key_encrypted, team_id into enc, tid from public.panels where id = p_panel;
  if tid is null then raise exception 'panel not found'; end if;
  if not public.is_team_member(tid) then raise exception 'not a member of this team'; end if;
  return pgp_sym_decrypt(enc, public.feather_master_key());
end; $$;

grant execute on function public.create_panel(uuid, text, text, text) to authenticated;
grant execute on function public.panel_api_key(uuid)                  to authenticated;
