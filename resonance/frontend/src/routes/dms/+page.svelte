<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/dms/+page.svelte
  // Direct Messages inbox. Left panel = conversation list; right = thread.
  // Messages are E2E encrypted; we render a placeholder for ciphertext here.
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import {
    listConversations,
    listDms,
    sendDm,
    type ConversationOut,
    type DmOut,
  } from '$lib/api/dms';
  import { connectPersonalWS, personalEvents } from '$stores/personal_ws';

  let conversations = $state<ConversationOut[]>([]);
  let activePartner = $state<string | null>(null);
  let messages = $state<DmOut[]>([]);
  let draft = $state('');
  let loading = $state(true);

  // Decrypt placeholder: in production, decrypt each message with X25519 ECDH.
  function renderCiphertext(ct: string): string {
    return `🔒 رسالة مشفّرة (${ct.length} بايت)`;
  }

  async function refreshConversations() {
    if (!$session.userId) return;
    try {
      conversations = await listConversations($session.userId);
    } catch (e: any) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  async function openConversation(partnerId: string) {
    if (!$session.userId) return;
    activePartner = partnerId;
    try {
      messages = await listDms($session.userId, partnerId);
    } catch (e: any) {
      console.error(e);
    }
  }

  async function handleSend(e: Event) {
    e.preventDefault();
    if (!$session.userId || !activePartner || !draft.trim()) return;
    try {
      // In production: generate ephemeral X25519 keypair, encrypt with ECDH,
      // send ciphertext + ephemeral pubkey. For demo: send as "encrypted" placeholder.
      const encoder = new TextEncoder();
      const ct = btoa(String.fromCharCode(...encoder.encode(draft)));
      const ephemeral = btoa('a'.repeat(32)); // placeholder
      await sendDm($session.userId, activePartner, ct, ephemeral);
      draft = '';
      await openConversation(activePartner);
      await refreshConversations();
    } catch (e: any) {
      console.error(e);
    }
  }

  onMount(() => {
    if ($session.userId) {
      connectPersonalWS($session.userId);
      refreshConversations();
    }
  });

  // Listen for new DM events.
  $effect(() => {
    const evt = $personalEvents;
    if (evt?.type === 'dm:new' && activePartner === evt.from_user_id) {
      if ($session.userId) openConversation(activePartner!);
    } else if (evt?.type === 'dm:new') {
      refreshConversations();
    }
  });

  function timeAgo(iso: string): string {
    const d = new Date(iso);
    const diff = Date.now() - d.getTime();
    const mins = Math.floor(diff / 60_000);
    const hours = Math.floor(diff / 3_600_000);
    const days = Math.floor(diff / 86_400_000);
    if (days > 0) return `${days}ي`;
    if (hours > 0) return `${hours}س`;
    if (mins > 0) return `${mins}د`;
    return 'دلوقتي';
  }
</script>

<main class="max-w-5xl mx-auto py-6 px-4 h-[calc(100vh-60px)]">
  <h1 class="font-serif text-2xl text-egyptian-gold-400 mb-4">الرسائل المباشرة</h1>

  <div class="grid grid-cols-1 md:grid-cols-3 gap-4 h-full">
    <!-- Conversations list -->
    <aside class="md:col-span-1 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20 overflow-hidden flex flex-col">
      <div class="p-3 border-b border-egyptian-gold-400/10 text-xs uppercase tracking-wider text-papyrus-300">
        المحادثات
      </div>
      <div class="flex-1 overflow-y-auto">
        {#if loading}
          <div class="p-4 text-center text-papyrus-400 text-sm">بيتحمّل...</div>
        {:else if conversations.length === 0}
          <div class="p-4 text-center text-papyrus-400 text-sm">
            مفيش محادثات لسه. ابدأ محادثة من ملف أي حد.
          </div>
        {:else}
          {#each conversations as c}
            <button
              on:click={() => openConversation(c.partner_id)}
              class="w-full text-right p-3 hover:bg-astral-blue-300/50 transition border-b border-egyptian-gold-400/5 {activePartner === c.partner_id ? 'bg-astral-blue-300/60' : ''}"
            >
              <div class="flex justify-between items-center">
                <span class="font-serif text-papyrus-100">{c.partner_username}</span>
                {#if c.unread_count > 0}
                  <span class="bg-egyptian-gold-400 text-astral-blue-700 text-xs font-bold rounded-full px-2 py-0.5">
                    {c.unread_count}
                  </span>
                {/if}
              </div>
              <div class="text-xs text-papyrus-400 mt-1">{timeAgo(c.last_message_at)}</div>
            </button>
          {/each}
        {/if}
      </div>
    </aside>

    <!-- Thread -->
    <section class="md:col-span-2 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20 flex flex-col">
      {#if activePartner}
        <div class="p-3 border-b border-egyptian-gold-400/10">
          <span class="font-serif text-papyrus-100">
            {conversations.find((c) => c.partner_id === activePartner)?.partner_username ?? 'محادثة'}
          </span>
        </div>

        <div class="flex-1 overflow-y-auto p-4 space-y-2">
          {#each messages as m}
            <div class="flex {m.sender_id === $session.userId ? 'justify-end' : 'justify-start'}">
              <div
                class="max-w-[70%] p-3 rounded-2xl text-sm {m.sender_id === $session.userId
                  ? 'bg-egyptian-gold-400/20 text-papyrus-100'
                  : 'bg-astral-blue-500/60 text-papyrus-100'}"
              >
                {renderCiphertext(m.ciphertext_b64)}
                <div class="text-xs text-papyrus-400 mt-1">{timeAgo(m.created_at)}</div>
              </div>
            </div>
          {/each}
        </div>

        <form on:submit={handleSend} class="p-3 border-t border-egyptian-gold-400/10 flex gap-2">
          <input
            bind:value={draft}
            placeholder="اكتب رسالتك..."
            class="flex-1 bg-astral-blue-500 text-papyrus-50 px-3 py-2 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400"
          />
          <button
            type="submit"
            disabled={!draft.trim()}
            class="px-4 py-2 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold disabled:opacity-50"
          >
            إرسال
          </button>
        </form>
      {:else}
        <div class="flex-1 flex items-center justify-center text-papyrus-400 text-sm">
          اختر محادثة من اليمين عشان تبدأ
        </div>
      {/if}
    </section>
  </div>
</main>
