import { createClient } from "@supabase/supabase-js";

// Feather's cloud backend. The anon key is designed to be shipped inside
// client apps — data is protected by Row-Level Security in the database, not
// by keeping this key secret. See supabase/0001_foundation.sql and
// docs/CLOUD-SETUP.md.
export const SUPABASE_URL = "https://unxaooxeopyqmkovaeqq.supabase.co";
export const SUPABASE_ANON_KEY =
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVueGFvb3hlb3B5cW1rb3ZhZXFxIiwicm9sZSI6ImFub24iLCJpYXQiOjE3ODQ2NjY4MDMsImV4cCI6MjEwMDI0MjgwM30.uX7jV26CSMlLdkE5RFtkuzLu8KYP0axa3s7Pvd7uidE";

export const supabase = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
  auth: {
    persistSession: true,
    autoRefreshToken: true,
    // The Tauri webview provides localStorage; sessions survive restarts.
    storageKey: "feather.auth",
  },
});
