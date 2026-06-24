// ============================================================================
// resonance-frontend/src/lib/api/goals.ts
// API client for "أُفقي" goals + "شموع الدعم" candles.
// ============================================================================

import { signedFetch, publicFetch } from './client';

export interface GoalOut {
  id: string;
  user_id: string;
  title: string;
  target: number;
  current: number;
  is_lit: boolean;
  created_at: string;
  lit_at: string | null;
}

export interface LightCandleResponse {
  new_current: number;
  is_lit: boolean;
  new_resonance_score: number;
}

export async function createGoal(
  userId: string,
  title: string,
  target: number,
): Promise<GoalOut> {
  return signedFetch<GoalOut>(userId, 'POST', '/goals', { title, target });
}

export async function listGoals(userId: string, targetUserId: string): Promise<GoalOut[]> {
  // `signedFetch` includes the signature, but listGoals is GET and we need
  // the requester's signature to be applied. Use signedFetch.
  return signedFetch<GoalOut[]>(userId, 'GET', `/goals/${targetUserId}`);
}

export async function lightCandle(
  userId: string,
  goalId: string,
): Promise<LightCandleResponse> {
  return signedFetch<LightCandleResponse>(userId, 'POST', `/goals/${goalId}/light`, {});
}

// Re-export publicFetch for callers that don't need a signature.
export { publicFetch };
