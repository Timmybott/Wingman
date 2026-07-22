// Reactive authentication state, backed by Supabase auth.

import type { Session, User } from "@supabase/supabase-js";
import { supabase } from "./supabase";

export const auth = $state({
  session: null as Session | null,
  user: null as User | null,
  loading: true,
});

/** Load the persisted session and subscribe to changes. Call once at startup. */
export async function initAuth(): Promise<void> {
  const { data } = await supabase.auth.getSession();
  auth.session = data.session;
  auth.user = data.session?.user ?? null;
  auth.loading = false;
  supabase.auth.onAuthStateChange((_event, session) => {
    auth.session = session;
    auth.user = session?.user ?? null;
  });
}

export async function signUp(
  email: string,
  password: string,
  displayName: string,
): Promise<{ needsConfirmation: boolean }> {
  const { data, error } = await supabase.auth.signUp({
    email,
    password,
    options: { data: { display_name: displayName } },
  });
  if (error) throw new Error(error.message);
  // When email confirmation is on, there is a user but no active session yet.
  return { needsConfirmation: !!data.user && !data.session };
}

export async function signIn(email: string, password: string): Promise<void> {
  const { error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw new Error(error.message);
}

export async function signOut(): Promise<void> {
  await supabase.auth.signOut();
}
