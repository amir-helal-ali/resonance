// ============================================================================
// resonance-frontend/src/lib/api/media.ts
// ============================================================================

import { signedFetch } from './client';

export interface MediaOut {
  id: string;
  pulse_id: string;
  kind: 'image' | 'video' | 'audio';
  mime_type: string;
  ciphertext_b64: string;
  iv_b64: string;
  width: number;
  height: number;
  duration_ms: number;
  created_at: string;
}

export async function uploadMedia(
  userId: string,
  data: {
    pulse_id: string;
    kind: 'image' | 'video' | 'audio';
    mime_type: string;
    ciphertext_b64: string;
    iv_b64: string;
    width?: number;
    height?: number;
    duration_ms?: number;
    original_sha256_b64: string;
  },
): Promise<MediaOut> {
  return signedFetch<MediaOut>(userId, 'POST', '/media', data);
}

export async function listMedia(userId: string, pulseId: string): Promise<MediaOut[]> {
  return signedFetch<MediaOut[]>(userId, 'GET', `/media?pulse_id=${pulseId}`);
}
