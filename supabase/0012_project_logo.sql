-- Feather cloud — project logo (M24).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0011
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Projects can carry a logo image URL, shown on the project page and in lists.
-- Editable by any team member through the existing projects_update policy.

alter table public.projects add column if not exists logo_url text;
