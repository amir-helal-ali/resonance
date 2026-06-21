// ============================================================================
// resonance-frontend/src/lib/api/client.ts
// Typed wrapper around `fetch` that signs every protected request with the
// Ed25519 keypair stored in IndexedDB.
// ============================================================================

import {
  bytesToBase64,
  base64ToBytes,
  loadPrivateKey,
  signRequest,
  type Ed25519Keypair,
} from '$crypto/blind_vault';

const API_BASE =
  (import.meta.env.VITE_PUBLIC_API_URL as string | undefined) ??
  'http://localhost:8080';

let cachedPubkey: Uint8Array | null = null;

/** Cache the public key in memory after first load to avoid IndexedDB hits. */
export async function getPubkey(userId: string): Promise<Uint8Array> {
  if (cachedPubkey) return cachedPubkey;
  // The public key is not sensitive — we can fetch it from /api/users/:id
  // and cache it. For this skeleton we assume it's stashed alongside the
  // private key in localStorage (NOT the private key — just the pub).
  const b64 = localStorage.getItem(`resonance:pubkey:${userId}`);
  if (!b64) throw new Error('public key not found; re-import needed');
  cachedPubkey = base64ToBytes(b64);
  return cachedPubkey;
}

export async function setPubkey(userId: string, pubkey: Uint8Array): Promise<void> {
  cachedPubkey = pubkey;
  localStorage.setItem(`resonance:pubkey:${userId}`, bytesToBase64(pubkey));
}

/** Make a signed request. Body is serialized as JSON. */
export async function signedFetch<T = any>(
  userId: string,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
): Promise<T> {
  const pub = await getPubkey(userId);
  const priv = await loadPrivateKey(userId);
  if (!priv) throw new Error('private key not unlocked');

  const bodyStr = body === undefined ? '' : JSON.stringify(body);
  const bodyBytes = new TextEncoder().encode(bodyStr);

  const { headers } = await signRequest({
    method,
    path,
    body: bodyBytes,
    privateKey: priv,
    publicKey: pub,
  });

  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers,
    body: method === 'GET' ? undefined : bodyStr,
  });

  if (!res.ok) {
    let msg = `HTTP ${res.status}`;
    try {
      const err = await res.json();
      msg = err.message ?? msg;
    } catch {
      // ignore JSON parse error
    }
    throw new Error(msg);
  }
  return res.json() as Promise<T>;
}

/** Make an unsigned (public) request. */
export async function publicFetch<T = any>(
  method: 'GET' | 'POST',
  path: string,
  body?: unknown,
): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: method === 'GET' ? undefined : JSON.stringify(body ?? {}),
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status}`);
  }
  return res.json() as Promise<T>;
}
