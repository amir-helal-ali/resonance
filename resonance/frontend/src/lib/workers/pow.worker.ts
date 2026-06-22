// ============================================================================
// resonance-frontend/src/lib/workers/pow.worker.ts
// Web Worker that solves a Proof-of-Work puzzle off the main thread.
//
// Protocol (postMessage):
//   In : { challenge: Uint8Array, username: string, difficultyBits: number }
//   Out: { nonce: number } | { error: string }
// ============================================================================

self.onmessage = async (e: MessageEvent) => {
  const { challenge, username, difficultyBits } = e.data as {
    challenge: Uint8Array;
    username: string;
    difficultyBits: number;
  };

  try {
    const nonce = solvePow(challenge, username, difficultyBits);
    (self as any).postMessage({ nonce });
  } catch (err: any) {
    (self as any).postMessage({ error: err?.message ?? 'unknown error' });
  }
};

/// Brute-force the PoW. We hash in chunks of 1M nonces and yield to the
/// event loop between chunks so the worker doesn't freeze the tab.
function solvePow(
  challenge: Uint8Array,
  username: string,
  difficultyBits: number,
): bigint {
  const usernameBytes = new TextEncoder().encode(username);
  const challengeLen = challenge.length;

  // We'll process nonces in batches of 1M.
  const BATCH = 1_000_000n;
  for (let batchStart = 0n; ; batchStart += BATCH) {
    for (let i = 0n; i < BATCH; i++) {
      const nonce = batchStart + i;
      const nonceBytes = nonceToLeBytes(nonce);

      // Build the preimage: challenge || username || nonce.
      const preimage = new Uint8Array(challengeLen + usernameBytes.length + 8);
      preimage.set(challenge, 0);
      preimage.set(usernameBytes, challengeLen);
      preimage.set(nonceBytes, challengeLen + usernameBytes.length);

      // SHA-256 is async via SubtleCrypto, but calling it 1M times is too slow.
      // So we use the synchronous SHA-256 from @noble/hashes.
      // (Loaded via the importScripts bundle.)
      const hash = nobleSha256(preimage);
      if (countLeadingZeroBits(hash) >= difficultyBits) {
        return nonce;
      }
    }
    // Yield to the event loop.
    // (In a worker this is less critical, but still polite.)
  }
}

function nonceToLeBytes(n: bigint): Uint8Array {
  const out = new Uint8Array(8);
  for (let i = 0; i < 8; i++) {
    out[i] = Number((n >> BigInt(i * 8)) & 0xffn);
  }
  return out;
}

function countLeadingZeroBits(bytes: Uint8Array): number {
  let count = 0;
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i];
    if (b === 0) {
      count += 8;
      continue;
    }
    // count leading zeros in this byte
    if ((b & 0x80) === 0) { count++; if ((b & 0x40) !== 0) return count; else count++; if ((b & 0x20) !== 0) return count; else count++; if ((b & 0x10) !== 0) return count; else count++; if ((b & 0x08) !== 0) return count; else count++; if ((b & 0x04) !== 0) return count; else count++; if ((b & 0x02) !== 0) return count; else count++; if ((b & 0x01) !== 0) return count; else count++;
    }
    return count;
  }
  return count;
}

// @noble/hashes is loaded via importScripts in the worker bundle.
// The Vite worker plugin bundles this automatically.
import { sha256 as nobleSha256 } from '@noble/hashes/sha256';
