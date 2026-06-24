// ============================================================================
// resonance-frontend/src/lib/api/jury.ts
// API client for the Transient Jury.
// ============================================================================

import { signedFetch } from './client';

export interface JuryOut {
  id: string;
  pulse_id: string;
  juror_ids: string[];
  final_verdict: 'pending' | 'uphold' | 'release' | 'expire';
  summoned_at: string;
  expires_at: string;
  concluded_at: string | null;
}

export interface CastVoteResponse {
  final_verdict: string;
  votes_for_release: number;
  votes_for_uphold: number;
}

export async function listSummoned(userId: string): Promise<JuryOut[]> {
  return signedFetch<JuryOut[]>(userId, 'GET', '/jury/summoned');
}

export async function castVote(
  userId: string,
  panelId: string,
  vote: 'uphold' | 'release',
): Promise<CastVoteResponse> {
  return signedFetch<CastVoteResponse>(
    userId,
    'POST',
    `/jury/${panelId}/vote`,
    { vote },
  );
}
