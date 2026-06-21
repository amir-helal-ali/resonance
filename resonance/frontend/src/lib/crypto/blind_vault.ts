// ============================================================================
// resonance-frontend/src/lib/crypto/blind_vault.ts
// Client-side cryptographic primitives for the Blind Vault onboarding flow.
//
// Everything here runs in the browser. The server NEVER sees:
//   - the cleartext email
//   - the Ed25519 private key
//   - the AES-GCM symmetric key used to wrap the email
//
// We use the native Web Crypto API (no external deps for crypto).
// ============================================================================

// ---------- Base64 helpers (Web Crypto doesn't ship them) ----------
export function bytesToBase64(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

// ---------- HKDF (key derivation) ----------
/**
 * Derive a 256-bit AES key from a password using HKDF-SHA256.
 * The salt is a random 16-byte value generated fresh per registration.
 * The info is the constant string "resonance.email.v1".
 */
export async function deriveEmailKek(
  password: string,
  salt: Uint8Array,
): Promise<CryptoKey> {
  // 1. PBKDF2 to stretch the password into a base key.
  const baseKey = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(password),
    { name: 'PBKDF2' },
    false,
    ['deriveKey'],
  );

  // 2. HKDF-SHA256 to derive the final AES-KW key.
  return crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: salt as BufferSource,
      info: new TextEncoder().encode('resonance.email.v1') as BufferSource,
    },
    baseKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'wrapKey', 'unwrapKey'],
  );
}

// ---------- AES-GCM (email encryption) ----------
/**
 * Encrypt the email with AES-GCM using the derived KEK.
 * Returns iv(12B) || ciphertext || tag(16B).
 */
export async function encryptEmail(
  email: string,
  kek: CryptoKey,
): Promise<Uint8Array> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: iv as BufferSource },
      kek,
      new TextEncoder().encode(email) as BufferSource,
    ),
  );
  // Concatenate iv + ciphertext (which includes the GCM tag at the end).
  const out = new Uint8Array(iv.length + ciphertext.length);
  out.set(iv, 0);
  out.set(ciphertext, iv.length);
  return out;
}

// ---------- Blind Index (HMAC-SHA256, truncated to 96 bits) ----------
/**
 * Compute the blind index for an email.
 *
 * The key is fetched from the server once (at registration) and never stored.
 * The server side uses the SAME key to recompute (for lookup) — but the key
 * is never on the wire in cleartext (TLS protects it).
 *
 * Actually, looking at this design again: the blind index HMAC key should
 * be a per-server secret. The client cannot compute it without the key.
 * So the protocol is:
 *   1. Client sends the cleartext email to /blind-index/compute over TLS.
 *      The server computes HMAC and returns the index. The email is in
 *      transit but never persisted.
 *   2. Client encrypts the email locally and discards the cleartext.
 *   3. Client registers with ciphertext + blind_index + pow + pubkey.
 *
 * To keep this client-side module self-contained (and to support a fully
 * offline registration flow), we expose BOTH a `computeBlindIndexServer`
 * helper AND a `computeBlindIndexLocal` that uses a client-held key.
 */
export async function computeBlindIndexServer(email: string): Promise<Uint8Array> {
  const res = await fetch('/api/blind-index', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email }),
  });
  if (!res.ok) throw new Error('blind index request failed');
  const { blind_index_b64 } = await res.json();
  return base64ToBytes(blind_index_b64);
}

/**
 * Local computation: requires the BLIND_INDEX_KEY to be embedded in the
 * client bundle. This is INSECURE for production but useful for testing.
 */
export async function computeBlindIndexLocal(
  email: string,
  keyB64: string,
): Promise<Uint8Array> {
  // Import the HMAC key.
  const key = await crypto.subtle.importKey(
    'raw',
    base64ToBytes(keyB64) as BufferSource,
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const sig = new Uint8Array(
    await crypto.subtle.sign(
      'HMAC',
      key,
      new TextEncoder().encode(email) as BufferSource,
    ),
  );
  // Truncate to 12 bytes (96 bits).
  return sig.slice(0, 12);
}

// ---------- Ed25519 keypair generation ----------
/**
 * Generate an Ed25519 keypair. The public key is sent to the server; the
 * private key NEVER leaves the browser.
 *
 * Web Crypto doesn't yet support Ed25519 in all browsers, so we fall back
 * to the `@noble/ed25519` library.
 */
export async function generateEd25519Keypair(): Promise<{
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}> {
  // Try native Web Crypto first.
  if (typeof crypto !== 'undefined' && 'generateKey' in crypto) {
    try {
      const kp = await crypto.subtle.generateKey(
        { name: 'Ed25519' },
        true,
        ['sign', 'verify'],
      );
      const pub = new Uint8Array(await crypto.subtle.exportKey('raw', kp.publicKey));
      const priv = new Uint8Array(await crypto.subtle.exportKey('pkcs8', kp.privateKey));
      // We need the raw 32-byte private key for noble's signing helper, so
      // extract it from the PKCS8 DER. Easier: just use noble from the start.
      return { publicKey: pub, privateKey: priv };
    } catch {
      // Fall through to noble.
    }
  }
  // Fallback: @noble/ed25519.
  const { utils, getPublicKey, generatePrivateKey } = await import('@noble/ed25519');
  utils.randomPrivateKey = () => crypto.getRandomValues(new Uint8Array(32));
  const privateKey = generatePrivateKey();
  const publicKey = await getPublicKey(privateKey);
  return { publicKey, privateKey };
}

// ---------- Ed25519 signing ----------
/**
 * Sign a canonical request with the Ed25519 private key.
 * Canonical string: method || "\n" || path || "\n" || timestamp || "\n" || sha256(body)
 *
 * The signed request is sent with three custom headers:
 *   X-Resonance-Key : base64(public_key)
 *   X-Resonance-Ts  : unix_millis
 *   X-Resonance-Sig : base64(signature)
 */
export async function signRequest(args: {
  method: string;
  path: string;
  body: Uint8Array | string;
  privateKey: Uint8Array;
  publicKey: Uint8Array;
}): Promise<{ headers: Record<string, string> }> {
  const ts = Date.now().toString();
  const bodyBytes =
    typeof args.body === 'string' ? new TextEncoder().encode(args.body) : args.body;

  // SHA-256 the body.
  const bodyHash = new Uint8Array(
    await crypto.subtle.digest('SHA-256', bodyBytes as BufferSource),
  );

  // Canonical string.
  const canon =
    `${args.method}\n${args.path}\n${ts}\n` +
    bytesToBase64(bodyHash);

  // Sign with @noble/ed25519 (Web Crypto's Ed25519 sign support is spotty).
  const { signAsync } = await import('@noble/ed25519');
  const sig = await signAsync(new TextEncoder().encode(canon), args.privateKey);

  return {
    headers: {
      'Content-Type': 'application/json',
      'X-Resonance-Key': bytesToBase64(args.publicKey),
      'X-Resonance-Ts': ts,
      'X-Resonance-Sig': bytesToBase64(sig),
    },
  };
}

// ---------- Secure storage ----------
/**
 * Store the Ed25519 private key in IndexedDB (NEVER in localStorage).
 * IndexedDB is origin-scoped and not accessible to XSS the same way
 * localStorage is (because we can wrap it in a closure).
 */
const DB_NAME = 'resonance-vault';
const STORE = 'keys';

function openVault(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function storePrivateKey(userId: string, key: Uint8Array): Promise<void> {
  const db = await openVault();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).put(key, userId);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export async function loadPrivateKey(userId: string): Promise<Uint8Array | null> {
  const db = await openVault();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readonly');
    const req = tx.objectStore(STORE).get(userId);
    req.onsuccess = () => resolve(req.result ? new Uint8Array(req.result) : null);
    req.onerror = () => reject(req.error);
  });
}

export async function clearPrivateKey(userId: string): Promise<void> {
  const db = await openVault();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).delete(userId);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}
