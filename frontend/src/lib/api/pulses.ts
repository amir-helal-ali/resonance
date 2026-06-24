// ============================================================================
// resonance-frontend/src/lib/api/pulses.ts
// API client for pulse creation + interactions (echo, save, comment, report).
// ============================================================================

import { signedFetch } from './client';

export interface CreatePulseResponse {
  pulse_id: string;
  lifecycle: 'glow' | 'linger' | 'evaporated';
  created_at: string;
}

export interface InteractionResponse {
  interaction_id: string;
  new_resonance_score: number;
}

export interface ReportResponse {
  report_id: string;
  jury_summoned: boolean;
}

export async function createPulse(
  userId: string,
  ciphertextB64: string,
  wrappedKeyB64: string,
  isPreserved: boolean,
): Promise<CreatePulseResponse> {
  return signedFetch<CreatePulseResponse>(userId, 'POST', '/pulses', {
    ciphertext: ciphertextB64,
    wrapped_key: wrappedKeyB64,
    is_preserved: isPreserved,
  });
}

export async function echoPulse(userId: string, pulseId: string): Promise<InteractionResponse> {
  return signedFetch<InteractionResponse>(userId, 'POST', `/pulses/${pulseId}/echo`, {});
}

export async function savePulse(userId: string, pulseId: string): Promise<InteractionResponse> {
  return signedFetch<InteractionResponse>(userId, 'POST', `/pulses/${pulseId}/save`, {});
}

export async function commentPulse(
  userId: string,
  pulseId: string,
  ciphertextB64: string,
): Promise<InteractionResponse> {
  return signedFetch<InteractionResponse>(
    userId,
    'POST',
    `/pulses/${pulseId}/comment`,
    { ciphertext_b64: ciphertextB64 },
  );
}

export async function reportPulse(
  userId: string,
  pulseId: string,
  reason: string,
): Promise<ReportResponse> {
  return signedFetch<ReportResponse>(
    userId,
    'POST',
    `/pulses/${pulseId}/report`,
    { reason },
  );
}
