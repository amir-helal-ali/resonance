<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/Composer.svelte
  // The composer for a new "نبضة" (pulse).
  //
  // Flow:
  //   1. User types the pulse text.
  //   2. Generate a per-pulse AES-GCM key (Web Crypto).
  //   3. Encrypt the text → ciphertext.
  //   4. Wrap the key under the user's HKDF-derived KEK (password).
  //      For demo we skip wrapping and just send the raw key.
  //   5. POST /pulses with ciphertext + wrapped_key + is_preserved.
  //   6. The new pulse appears in the LiveFeed via the WS broadcast.
  // ============================================================================

  import { createPulse } from '$lib/api/pulses';
  import { session } from '$stores/session';

  let text = $state('');
  let preserve = $state(false);
  let posting = $state(false);
  let error = $state<string | null>(null);

  const MAX_CHARS = 500;

  async function handlePost(e: Event) {
    e.preventDefault();
    if (!$session.userId || !$session.unlocked) {
      error = 'افتح قاعتك الأول';
      return;
    }
    if (text.length === 0) return;

    posting = true;
    error = null;
    try {
      // 1. Generate a per-pulse AES-GCM key.
      const key = await crypto.subtle.generateKey(
        { name: 'AES-GCM', length: 256 },
        true,
        ['encrypt', 'decrypt'],
      );
      // 2. Encrypt the text.
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const ct = new Uint8Array(
        await crypto.subtle.encrypt(
          { name: 'AES-GCM', iv },
          key,
          new TextEncoder().encode(text),
        ),
      );
      // 3. Combine iv + ciphertext.
      const combined = new Uint8Array(iv.length + ct.length);
      combined.set(iv, 0);
      combined.set(ct, iv.length);

      // 4. Export the raw key for storage (in production: wrap it under the
      //    user's HKDF-derived KEK first).
      const rawKey = new Uint8Array(await crypto.subtle.exportKey('raw', key));

      // 5. Convert to base64.
      const ctB64 = bytesToBase64(combined);
      const wkB64 = bytesToBase64(rawKey);

      // 6. POST.
      await createPulse($session.userId, ctB64, wkB64, preserve);
      text = '';
      // The LiveFeed will pick this up via the WS broadcast automatically.
    } catch (err: any) {
      error = err?.message ?? 'فشل إرسال النبضة';
    } finally {
      posting = false;
    }
  }

  function bytesToBase64(bytes: Uint8Array): string {
    let bin = '';
    for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    return btoa(bin);
  }

  const remaining = $derived(MAX_CHARS - text.length);
</script>

<form on:submit={handlePost} class="mb-6 p-4 rounded-2xl bg-astral-blue-300/60 border border-egyptian-gold-400/20">
  <textarea
    bind:value={text}
    maxlength={MAX_CHARS}
    rows="3"
    placeholder="إيه اللي بينبض جواك؟"
    class="w-full bg-astral-blue-500 text-papyrus-50 p-3 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 resize-none"
  ></textarea>

  {#if error}
    <div class="mt-2 text-sm text-red-300">{error}</div>
  {/if}

  <div class="flex items-center justify-between mt-3">
    <div class="flex items-center gap-4">
      <label class="flex items-center gap-2 text-sm text-papyrus-200 cursor-pointer">
        <input type="checkbox" bind:checked={preserve} class="accent-egyptian-gold-400" />
        تخليد
      </label>
      <span class="text-xs {remaining < 50 ? 'text-egyptian-gold-300' : 'text-papyrus-400'}">
        {remaining}
      </span>
    </div>
    <button
      type="submit"
      disabled={posting || text.length === 0}
      class="px-5 py-2 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold disabled:opacity-50 hover:bg-egyptian-gold-300 transition"
    >
      {posting ? 'بيتبعت...' : 'نبض'}
    </button>
  </div>
</form>
