-- Feather cloud — image storage for avatars and logos (M39).
--
-- Run this ONCE in the Supabase SQL editor AFTER 0001–0013
-- (Dashboard → SQL Editor → New query → paste → Run). Idempotent.
--
-- Creates a public "images" bucket. Any signed-in team member can upload an
-- avatar or logo; the files are world-readable (public URLs are stored in
-- profiles.avatar_url / teams.logo_url / projects.logo_url). Images are small,
-- non-sensitive, and referenced by unguessable paths.

insert into storage.buckets (id, name, public)
values ('images', 'images', true)
on conflict (id) do update set public = true;

-- Public read (the bucket is public; this makes the intent explicit).
drop policy if exists "feather images read" on storage.objects;
create policy "feather images read" on storage.objects
  for select using (bucket_id = 'images');

-- Signed-in users may upload and replace images in this bucket.
drop policy if exists "feather images insert" on storage.objects;
create policy "feather images insert" on storage.objects
  for insert to authenticated with check (bucket_id = 'images');

drop policy if exists "feather images update" on storage.objects;
create policy "feather images update" on storage.objects
  for update to authenticated
  using (bucket_id = 'images')
  with check (bucket_id = 'images');
