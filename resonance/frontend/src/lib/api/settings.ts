// ============================================================================
// resonance-frontend/src/lib/api/settings.ts
// ============================================================================

import { signedFetch } from './client';

export interface UpdateProfileResponse {
  updated: string[];
}

export interface BlockOut {
  user_id: string;
  username: string;
  created_at: string;
}

export interface SavedPulseOut {
  pulse_id: string;
  author_id: string;
  ciphertext_b64: string;
  created_at: string;
  personal_note: string;
  saved_at: string;
}

export async function updateProfile(
  userId: string,
  patch: { imprint?: string; horizon?: string },
): Promise<UpdateProfileResponse> {
  return signedFetch<UpdateProfileResponse>(userId, 'PATCH', '/settings/profile', patch);
}

export async function rotateKey(
  userId: string,
  newPublicKeyB64: string,
  authorizationSigB64: string,
): Promise<void> {
  await signedFetch(userId, 'POST', '/settings/rotate-key', {
    new_public_key_b64: newPublicKeyB64,
    authorization_sig_b64: authorizationSigB64,
  });
}

export async function deleteAccount(userId: string): Promise<void> {
  await signedFetch(userId, 'DELETE', '/settings/account', {});
}

export async function listBlocks(userId: string): Promise<BlockOut[]> {
  return signedFetch<BlockOut[]>(userId, 'GET', '/settings/blocks');
}

export async function blockUser(userId: string, targetId: string): Promise<void> {
  await signedFetch(userId, 'POST', '/settings/blocks', { user_id: targetId });
}

export async function unblockUser(userId: string, targetId: string): Promise<void> {
  await signedFetch(userId, 'DELETE', `/settings/blocks/${targetId}`);
}

export async function listSaved(userId: string): Promise<SavedPulseOut[]> {
  return signedFetch<SavedPulseOut[]>(userId, 'GET', '/settings/saved');
}

export async function saveBookmark(
  userId: string,
  pulseId: string,
  personalNote = '',
): Promise<void> {
  await signedFetch(userId, 'POST', `/pulses/${pulseId}/save-bookmark`, {
    personal_note: personalNote,
  });
}

export async function removeBookmark(userId: string, pulseId: string): Promise<void> {
  await signedFetch(userId, 'DELETE', `/pulses/${pulseId}/save-bookmark`);
}
