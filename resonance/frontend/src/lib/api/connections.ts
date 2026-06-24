// ============================================================================
// resonance-frontend/src/lib/api/connections.ts
// API client for the resonance "sync" relationship + Co-Resonance suggestions.
// ============================================================================

import { signedFetch } from './client';

export interface SyncResponse {
  new_score: number;
  aura_now_visible: boolean;
}

export interface ConnectionOut {
  target_user_id: string;
  resonance_score: number;
  last_interaction_at: string;
}

export interface SuggestionEntry {
  user_id: string;
  username: string;
  jaccard: number;
}

export async function syncConnection(
  userId: string,
  targetUserId: string,
  strength?: number,
): Promise<SyncResponse> {
  return signedFetch<SyncResponse>(userId, 'POST', '/connections/sync', {
    target_user_id: targetUserId,
    strength,
  });
}

export async function listConnections(userId: string): Promise<ConnectionOut[]> {
  return signedFetch<ConnectionOut[]>(userId, 'GET', '/connections');
}

export async function suggestConnections(userId: string): Promise<SuggestionEntry[]> {
  return signedFetch<SuggestionEntry[]>(userId, 'GET', '/connections/suggest');
}

export async function unsync(userId: string, targetUserId: string): Promise<void> {
  await signedFetch(userId, 'DELETE', `/connections/${targetUserId}`);
}
