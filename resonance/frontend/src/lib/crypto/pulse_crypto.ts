// ============================================================================
// resonance-frontend/src/lib/crypto/pulse_crypto.ts
// Real per-pulse AES-GCM decryption + Ed25519→X25519 conversion for DMs.
//
// The flow:
//   1. User unlocks their Ed25519 keypair (from IndexedDB) with a password.
//   2. To decrypt a pulse: fetch the wrapped_key from the server, unwrap it
//      with the user's KEK (HKDF-derived from password), then AES-GCM decrypt
//      the pulse ciphertext.
//   3. For DMs: convert Ed25519 → X25519 (via @noble/curves), perform ECDH
//      with the recipient's static X25519 public key, derive a per-message
//      AES-GCM key, encrypt the message.
// ============================================================================

import { bytesToBase64, base64ToBytes } from './blind_vault';

// ---------- Per-pulse AES-GCM decryption ----------

/**
 * Decrypt a pulse's body using the wrapped per-pulse key.
 *
 * @param ciphertextB64  base64(iv || ciphertext || tag) from the server
 * @param wrappedKeyB64  base64 of the wrapped per-pulse key (AES-KW under user KEK)
 * @param kek            the user's KEK (CryptoKey, derived from password via HKDF)
 */
export async function decryptPulse(
  ciphertextB64: string,
  wrappedKeyB64: string,
  kek: CryptoKey,
): Promise<string> {
  // 1. Unwrap the per-pulse AES-GCM key (AES-KW).
  const wrappedKeyBytes = base64ToBytes(wrappedKeyB64);
  const pulseKey = await crypto.subtle.unwrapKey(
    'raw',                    // format of the wrapped key
    wrappedKeyBytes as BufferSource,
    kek,                      // unwrapping key
    { name: 'AES-KW' },       // algorithm used to wrap
    { name: 'AES-GCM', length: 256 }, // algorithm of the unwrapped key
    false,                    // not extractable
    ['decrypt'],
  );

  // 2. Split iv (12 bytes) from ciphertext.
  const combined = base64ToBytes(ciphertextB64);
  if (combined.length < 12 + 16) {
    throw new Error('ciphertext too short');
  }
  const iv = combined.slice(0, 12);
  const ct = combined.slice(12);

  // 3. AES-GCM decrypt.
  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: iv as BufferSource },
    pulseKey,
    ct as BufferSource,
  );
  return new TextDecoder().decode(plaintext);
}

/**
 * Encrypt a pulse's body using a freshly generated per-pulse key.
 * Returns { ciphertext_b64, wrapped_key_b64 } for the server.
 */
export async function encryptPulse(
  text: string,
  kek: CryptoKey,
): Promise<{ ciphertextB64: string; wrappedKeyB64: string }> {
  // 1. Generate per-pulse AES-GCM key.
  const pulseKey = await crypto.subtle.generateKey(
    { name: 'AES-GCM', length: 256 },
    true,
    ['encrypt', 'decrypt'],
  );

  // 2. Encrypt the text.
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: iv as BufferSource },
      pulseKey,
      new TextEncoder().encode(text) as BufferSource,
    ),
  );
  const combined = new Uint8Array(iv.length + ct.length);
  combined.set(iv, 0);
  combined.set(ct, iv.length);

  // 3. Wrap the pulse key under the user's KEK (AES-KW).
  const wrappedKey = new Uint8Array(
    await crypto.subtle.wrapKey('raw', pulseKey, kek, { name: 'AES-KW' }),
  );

  return {
    ciphertextB64: bytesToBase64(combined),
    wrappedKeyB64: bytesToBase64(wrappedKey),
  };
}

// ---------- X25519 ECDH for DMs ----------

/**
 * Convert an Ed25519 private key to X25519 (for ECDH in DMs).
 * Uses the @noble/curves library.
 */
export async function ed25519ToX25519Private(
  ed25519Priv: Uint8Array,
): Promise<Uint8Array> {
  const { x25519 } = await import('@noble/curves/x25519');
  // The conversion uses SHA-512 of the Ed25519 seed, then clamping per RFC 7748.
  // @noble/curves exposes `x25519.utils.edwardsToMontgomeryPriv()`.
  return x25519.utils.edwardsToMontgomeryPriv(ed25519Priv);
}

/**
 * Convert an Ed25519 public key to X25519 (for ECDH in DMs).
 */
export async function ed25519ToX25519Public(
  ed25519Pub: Uint8Array,
): Promise<Uint8Array> {
  const { x25519 } = await import('@noble/curves/x25519');
  return x25519.utils.edwardsToMontgomeryPub(ed25519Pub);
}

/**
 * Encrypt a DM using X25519 ECDH with the recipient's static public key.
 *
 * Returns { ciphertext_b64, ephemeral_pubkey_b64 } for the server.
 */
export async function encryptDm(
  text: string,
  myEd25519Priv: Uint8Array,
  recipientEd25519Pub: Uint8Array,
): Promise<{ ciphertextB64: string; ephemeralPubkeyB64: string }> {
  const { x25519 } = await import('@noble/curves/x25519');

  // 1. Generate an ephemeral X25519 keypair.
  const ephemeralPriv = x25519.utils.randomPrivateKey();
  const ephemeralPub = x25519.getPublicKey(ephemeralPriv);

  // 2. Convert recipient's Ed25519 pubkey → X25519.
  const recipientX25519 = await ed25519ToX25519Public(recipientEd25519Pub);

  // 3. ECDH: derive shared secret.
  const sharedSecret = x25519.getSharedSecret(ephemeralPriv, recipientX25519);

  // 4. HKDF the shared secret into an AES-GCM key.
  const dmKey = await crypto.subtle.importKey(
    'raw',
    sharedSecret as BufferSource,
    { name: 'HKDF' },
    false,
    ['deriveKey'],
  );
  const aesKey = await crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: ephemeralPub as BufferSource, // include ephemeral pubkey in the KDF
      info: new TextEncoder().encode('resonance.dm.v1') as BufferSource,
    },
    dmKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );

  // 5. AES-GCM encrypt.
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: iv as BufferSource },
      aesKey,
      new TextEncoder().encode(text) as BufferSource,
    ),
  );
  const combined = new Uint8Array(iv.length + ct.length);
  combined.set(iv, 0);
  combined.set(ct, iv.length);

  return {
    ciphertextB64: bytesToBase64(combined),
    ephemeralPubkeyB64: bytesToBase64(ephemeralPub),
  };
}

/**
 * Decrypt a DM using X25519 ECDH with the sender's ephemeral public key.
 */
export async function decryptDm(
  ciphertextB64: string,
  ephemeralPubkeyB64: string,
  myEd25519Priv: Uint8Array,
): Promise<string> {
  const { x25519 } = await import('@noble/curves/x25519');

  // 1. Convert my Ed25519 priv → X25519.
  const myX25519 = await ed25519ToX25519Private(myEd25519Priv);

  // 2. Decode ephemeral pubkey.
  const ephemeralPub = base64ToBytes(ephemeralPubkeyB64);

  // 3. ECDH.
  const sharedSecret = x25519.getSharedSecret(myX25519, ephemeralPub);

  // 4. HKDF (same params as encrypt).
  const dmKey = await crypto.subtle.importKey(
    'raw',
    sharedSecret as BufferSource,
    { name: 'HKDF' },
    false,
    ['deriveKey'],
  );
  const aesKey = await crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: ephemeralPub as BufferSource,
      info: new TextEncoder().encode('resonance.dm.v1') as BufferSource,
    },
    dmKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );

  // 5. AES-GCM decrypt.
  const combined = base64ToBytes(ciphertextB64);
  const iv = combined.slice(0, 12);
  const ct = combined.slice(12);
  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: iv as BufferSource },
    aesKey,
    ct as BufferSource,
  );
  return new TextDecoder().decode(plaintext);
}

// ---------- KEK derivation (password → AES-KW key) ----------

/**
 * Derive an AES-KW KEK from password + salt via PBKDF2 + HKDF.
 * Same as deriveEmailKek but for wrapping/unwrapping per-pulse keys.
 */
export async function deriveKek(
  password: string,
  salt: Uint8Array,
): Promise<CryptoKey> {
  const baseKey = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(password) as BufferSource,
    { name: 'PBKDF2' },
    false,
    ['deriveKey'],
  );
  return crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: salt as BufferSource,
      info: new TextEncoder().encode('resonance.kek.v1') as BufferSource,
    },
    baseKey,
    { name: 'AES-KW', length: 256 },
    false,
    ['wrapKey', 'unwrapKey'],
  );
}
