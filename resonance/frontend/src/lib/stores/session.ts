// ============================================================================
// resonance-frontend/src/lib/stores/session.ts
// Svelte stores for session state.
// ============================================================================

import { writable } from 'svelte/store';

export interface SessionState {
  userId: string | null;
  username: string | null;
  /** True when the Ed25519 private key has been unlocked for this session. */
  unlocked: boolean;
}

export const session = writable<SessionState>({
  userId: null,
  username: null,
  unlocked: false,
});

export function setSession(userId: string, username: string) {
  session.set({ userId, username, unlocked: true });
}

export function clearSession() {
  session.set({ userId: null, username: null, unlocked: false });
}
