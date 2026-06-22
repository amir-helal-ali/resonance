// ============================================================================
// resonance-frontend/src/lib/api/presence.ts
// API client for the Presence & Traces endpoints.
// ============================================================================

import { signedFetch } from './client';

export interface PresenceEntry {
  user_id: string;
  username: string;
}

export interface PulsePresenceResponse {
  aura_visible: boolean;
  pulsing_now_count: number;
}

export interface TraceOut {
  id: string;
  kind: 'named' | 'anonymous';
  visitor_username: string | null;
  created_at: string;
  expires_at: string;
}

export async function pulsePresence(
  userId: string,
  targetUserId: string,
  leaveNamedTrace: boolean,
): Promise<PulsePresenceResponse> {
  return signedFetch<PulsePresenceResponse>(userId, 'POST', '/presence/pulse', {
    target_user_id: targetUserId,
    leave_named_trace: leaveNamedTrace,
  });
}

export async function listPresence(
  userId: string,
  targetUserId: string,
): Promise<PresenceEntry[]> {
  return signedFetch<PresenceEntry[]>(userId, 'GET', `/presence/${targetUserId}`);
}

export async function listMyTraces(userId: string): Promise<TraceOut[]> {
  return signedFetch<TraceOut[]>(userId, 'GET', '/traces');
}
