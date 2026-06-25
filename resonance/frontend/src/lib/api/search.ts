// ============================================================================
// resonance-frontend/src/lib/api/search.ts
// ============================================================================

import { signedFetch } from './client';

export type SearchResult =
  | { type: 'user'; user_id: string; username: string; imprint_preview: string }
  | { type: 'hashtag'; hashtag_id: string; tag: string; pulse_count: number };

export async function search(
  userId: string,
  q: string,
  kind: 'users' | 'hashtags' | 'all' = 'all',
  limit = 20,
): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q, kind, limit: limit.toString() });
  return signedFetch<SearchResult[]>(userId, 'GET', `/search?${params}`);
}
