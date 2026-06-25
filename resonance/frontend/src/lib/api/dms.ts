// ============================================================================
// resonance-frontend/src/lib/api/dms.ts
// ============================================================================

import { signedFetch } from './client';

export interface SendDmResponse {
  message_id: string;
  created_at: string;
  expires_at: string;
}

export interface DmOut {
  id: string;
  sender_id: string;
  ciphertext_b64: string;
  ephemeral_pubkey_b64: string;
  read_at: string | null;
  created_at: string;
  expires_at: string;
}

export interface ConversationOut {
  partner_id: string;
  partner_username: string;
  unread_count: number;
  last_message_at: string;
}

export async function sendDm(
  userId: string,
  recipientId: string,
  ciphertextB64: string,
  ephemeralPubkeyB64: string,
): Promise<SendDmResponse> {
  return signedFetch<SendDmResponse>(userId, 'POST', '/dms', {
    recipient_id: recipientId,
    ciphertext_b64: ciphertextB64,
    ephemeral_pubkey_b64: ephemeralPubkeyB64,
  });
}

export async function listDms(
  userId: string,
  withUser: string,
  limit = 50,
  before?: string,
): Promise<DmOut[]> {
  const params = new URLSearchParams({
    with_user: withUser,
    limit: limit.toString(),
  });
  if (before) params.set('before', before);
  return signedFetch<DmOut[]>(userId, 'GET', `/dms?${params}`);
}

export async function listConversations(userId: string): Promise<ConversationOut[]> {
  return signedFetch<ConversationOut[]>(userId, 'GET', '/dms/conversations');
}
