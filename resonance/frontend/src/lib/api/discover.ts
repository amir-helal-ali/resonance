// ============================================================================
// resonance-frontend/src/lib/api/discover.ts
// ============================================================================

import { signedFetch } from './client';

export interface TrendingHashtag {
  hashtag_id: string;
  tag: string;
  pulse_count: number;
  unique_authors: number;
}

export interface SuggestedUser {
  user_id: string;
  username: string;
  imprint_preview: string;
  jaccard: number;
  badges: string[];
}

export interface UserProfileOut {
  user_id: string;
  username: string;
  imprint: string;
  horizon: string;
  created_at: string;
  badges: string[];
}

export async function getTrending(userId: string): Promise<TrendingHashtag[]> {
  return signedFetch<TrendingHashtag[]>(userId, 'GET', '/discover/trending');
}

export async function getSuggestedUsers(userId: string): Promise<SuggestedUser[]> {
  return signedFetch<SuggestedUser[]>(userId, 'GET', '/discover/suggested-users');
}

export async function lookupByUsername(
  userId: string,
  username: string,
): Promise<UserProfileOut> {
  return signedFetch<UserProfileOut>(
    userId,
    'GET',
    `/users/by-username/${encodeURIComponent(username)}`,
  );
}
