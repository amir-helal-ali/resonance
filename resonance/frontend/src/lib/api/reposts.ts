// ============================================================================
// resonance-frontend/src/lib/api/reposts.ts
// ============================================================================

import { signedFetch } from './client';

export interface RepostResponse {
  repost_pulse_id: string;
  is_quote: boolean;
  created_at: string;
}

export interface RepostOut {
  repost_pulse_id: string;
  author_id: string;
  is_quote: boolean;
  created_at: string;
}

export async function repost(
  userId: string,
  originalPulseId: string,
  quoteCiphertextB64?: string,
  quoteWrappedKeyB64?: string,
): Promise<RepostResponse> {
  return signedFetch<RepostResponse>(userId, 'POST', '/pulses/repost', {
    original_pulse_id: originalPulseId,
    quote_ciphertext_b64: quoteCiphertextB64,
    quote_wrapped_key_b64: quoteWrappedKeyB64,
  });
}

export async function listReposts(userId: string, pulseId: string): Promise<RepostOut[]> {
  return signedFetch<RepostOut[]>(userId, 'GET', `/pulses/${pulseId}/reposts`);
}
