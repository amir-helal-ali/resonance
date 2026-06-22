// ============================================================================
// resonance-frontend/src/lib/workers/pow.client.ts
// Thin wrapper that instantiates the PoW Web Worker and returns a promise.
// ============================================================================

import type { Worker } from '$types/worker';

export interface PowSolution {
  nonce: bigint;
}

/**
 * Solve a PoW puzzle in a Web Worker.
 *
 * @param challenge  32-byte server-issued challenge.
 * @param username   the requested username.
 * @param difficultyBits number of leading zero bits required.
 * @returns the nonce that solves the puzzle.
 */
export async function solvePowInWorker(
  challenge: Uint8Array,
  username: string,
  difficultyBits: number,
): Promise<PowSolution> {
  const worker: Worker = new Worker(
    new URL('./pow.worker.ts', import.meta.url),
    { type: 'module' },
  );

  return new Promise((resolve, reject) => {
    worker.onmessage = (e: MessageEvent) => {
      const data = e.data as { nonce?: number; error?: string };
      worker.terminate();
      if (data.error) {
        reject(new Error(data.error));
      } else if (typeof data.nonce === 'number') {
        resolve({ nonce: BigInt(data.nonce) });
      } else {
        reject(new Error('invalid worker response'));
      }
    };
    worker.onerror = (err) => {
      worker.terminate();
      reject(err);
    };
    // We must post a copy of the challenge; the worker can't share memory.
    worker.postMessage(
      {
        challenge: challenge.slice(),
        username,
        difficultyBits,
      },
      [challenge.slice().buffer],
    );
  });
}
