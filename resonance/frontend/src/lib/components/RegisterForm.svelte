<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/RegisterForm.svelte
  // The Blind Vault Registration UI.
  //
  // Flow:
  //   1. User enters (username, email, password).
  //   2. We derive an AES-GCM KEK from (password, random salt) via HKDF.
  //   3. We encrypt the email locally — the cleartext never leaves the browser.
  //   4. We compute the blind index via a server endpoint (the server holds
  //      the HMAC key; the email is in TLS transit but never persisted).
  //   5. We generate an Ed25519 keypair; the private key goes to IndexedDB.
  //   6. We fetch a PoW challenge from the server and solve it in a Web Worker.
  //   7. We POST /register with ciphertext + blind_index + pow + pubkey.
  //   8. We POST /verify-otp after the user types the OTP from the blind relay.
  // ============================================================================

  import {
    bytesToBase64,
    base64ToBytes,
    deriveEmailKek,
    encryptEmail,
    computeBlindIndexServer,
    generateEd25519Keypair,
    storePrivateKey,
    setPubkey,
  } from '$crypto/blind_vault';
  import { solvePowInWorker } from '$lib/workers/pow.client';
  import { publicFetch, setSession } from '$lib/api/client';
  import { setSession as setSessionStore } from '$stores/session';

  // ---- Form state ----
  let username = $state('');
  let email = $state('');
  let password = $state('');
  let otp = $state('');
  let error = $state<string | null>(null);
  let phase = $state<'form' | 'pow' | 'otp' | 'done'>('form');
  let powProgress = $state(0n);
  let pendingChallengeId = $state<string | null>(null);
  let pendingUserId = $state<string | null>(null);

  // ---- Submit handler ----
  async function handleRegister(e: Event) {
    e.preventDefault();
    error = null;
    phase = 'pow';

    try {
      // 1. Generate Ed25519 keypair.
      const kp = await generateEd25519Keypair();

      // 2. Derive KEK and encrypt the email locally.
      const salt = crypto.getRandomValues(new Uint8Array(16));
      const kek = await deriveEmailKek(password, salt);
      const ciphertext = await encryptEmail(email, kek);

      // 3. Compute blind index (server-side HMAC).
      const blindIndex = await computeBlindIndexServer(email);

      // 4. Fetch PoW challenge.
      const challengeRes = await publicFetch<{ challenge: string; expires_in_secs: number }>(
        'GET',
        '/pow/challenge',
      );
      const [challengeId, challengeB64] = challengeRes.challenge.split(':');
      const challengeBytes = base64ToBytes(challengeB64);

      // 5. Solve PoW in a Web Worker.
      const { nonce } = await solvePowInWorker(challengeBytes, username, 20);

      // 6. POST /register.
      const regRes = await publicFetch<{ user_id: string; otp_challenge_id: string }>(
        'POST',
        '/register',
        {
          username,
          email_ciphertext: bytesToBase64(ciphertext),
          email_blind_index: bytesToBase64(blindIndex),
          public_key: bytesToBase64(kp.publicKey),
          pow: {
            challenge: challengeRes.challenge,
            nonce: Number(nonce),
          },
        },
      );

      // 7. Store the private key in IndexedDB.
      await storePrivateKey(regRes.user_id, kp.privateKey);
      await setPubkey(regRes.user_id, kp.publicKey);

      pendingUserId = regRes.user_id;
      pendingChallengeId = regRes.otp_challenge_id;
      phase = 'otp';
    } catch (err: any) {
      error = err?.message ?? 'registration failed';
      phase = 'form';
    }
  }

  // ---- OTP verify handler ----
  async function handleVerifyOtp(e: Event) {
    e.preventDefault();
    error = null;
    try {
      const res = await publicFetch<{ verified: boolean; user_id: string }>(
        'POST',
        '/verify-otp',
        {
          otp_challenge_id: pendingChallengeId,
          otp,
        },
      );
      if (!res.verified) throw new Error('OTP verification failed');
      setSessionStore(res.user_id, username);
      phase = 'done';
    } catch (err: any) {
      error = err?.message ?? 'OTP verify failed';
    }
  }
</script>

<div class="max-w-md mx-auto mt-16 p-8 rounded-2xl bg-astral-blue-300/60 border border-egyptian-gold-400/20">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2 text-center">صدى</h1>
  <p class="text-papyrus-200 text-sm text-center mb-6">
    عشان تأثر في النبض، سجّل دلوقتي
  </p>

  {#if error}
    <div class="mb-4 p-3 rounded bg-red-900/40 border border-red-500 text-red-200 text-sm">
      {error}
    </div>
  {/if}

  {#if phase === 'form'}
    <!-- ============================================================ -->
    <!-- PHASE 1: form                                                -->
    <!-- ============================================================ -->
    <form on:submit={handleRegister} class="space-y-4">
      <div>
        <label class="block text-sm text-papyrus-200 mb-1" for="username">اسم المستخدم</label>
        <input
          id="username"
          bind:value={username}
          required
          minlength="3"
          maxlength="32"
          class="w-full px-3 py-2 rounded bg-astral-blue-500 border border-egyptian-gold-400/30 text-papyrus-50 focus:outline-none focus:border-egyptian-gold-400"
        />
      </div>
      <div>
        <label class="block text-sm text-papyrus-200 mb-1" for="email">البريد الإلكتروني</label>
        <input
          id="email"
          type="email"
          bind:value={email}
          required
          class="w-full px-3 py-2 rounded bg-astral-blue-500 border border-egyptian-gold-400/30 text-papyrus-50 focus:outline-none focus:border-egyptian-gold-400"
        />
        <p class="text-xs text-papyrus-300 mt-1">
          يُشفّر في متصفحك. خوادمنا لا تراه أبداً.
        </p>
      </div>
      <div>
        <label class="block text-sm text-papyrus-200 mb-1" for="password">كلمة المرور</label>
        <input
          id="password"
          type="password"
          bind:value={password}
          required
          minlength="8"
          class="w-full px-3 py-2 rounded bg-astral-blue-500 border border-egyptian-gold-400/30 text-papyrus-50 focus:outline-none focus:border-egyptian-gold-400"
        />
        <p class="text-xs text-papyrus-300 mt-1">
          تُستخدم محلياً فقط لاشتقاق مفتاح تشفير بريدك.
        </p>
      </div>
      <button
        type="submit"
        class="w-full py-3 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold hover:bg-egyptian-gold-300 transition"
      >
        سجّل وابدأ النبض
      </button>
    </form>

  {:else if phase === 'pow'}
    <!-- ============================================================ -->
    <!-- PHASE 2: solving PoW                                         -->
    <!-- ============================================================ -->
    <div class="text-center py-8">
      <div class="inline-block w-12 h-12 border-4 border-egyptian-gold-400 border-t-transparent rounded-full animate-spin mb-4"></div>
      <p class="text-papyrus-200">بنشتغل على لغز إثبات العمل...</p>
      <p class="text-xs text-papyrus-400 mt-2">ده يحميصدى من البوتات.</p>
    </div>

  {:else if phase === 'otp'}
    <!-- ============================================================ -->
    <!-- PHASE 3: OTP verification                                    -->
    <!-- ============================================================ -->
    <form on:submit={handleVerifyOtp} class="space-y-4">
      <p class="text-sm text-papyrus-200 mb-2">
        بعتنا الكود على بريدك عبر قناة عمياء. اكتبه هنا.
      </p>
      <input
        bind:value={otp}
        required
        pattern="[0-9]{6}"
        maxlength="6"
        placeholder="000000"
        class="w-full px-3 py-3 rounded bg-astral-blue-500 border border-egyptian-gold-400/30 text-papyrus-50 text-center text-2xl tracking-widest focus:outline-none focus:border-egyptian-gold-400"
      />
      <button
        type="submit"
        class="w-full py-3 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold hover:bg-egyptian-gold-300 transition"
      >
        تأكيد
      </button>
    </form>

  {:else if phase === 'done'}
    <!-- ============================================================ -->
    <!-- PHASE 4: success                                             -->
    <!-- ============================================================ -->
    <div class="text-center py-8">
      <div class="text-5xl mb-4">✨</div>
      <h2 class="font-serif text-2xl text-egyptian-gold-400 mb-2">أهلاً بيكم في صدى</h2>
      <p class="text-papyrus-200 mb-6">قاعتك جاهزة. ابدأ النبض.</p>
      <a
        href="/feed"
        class="inline-block px-6 py-3 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold hover:bg-egyptian-gold-300 transition"
      >
        روح للخلاصة
      </a>
    </div>
  {/if}
</div>
