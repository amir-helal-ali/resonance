<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/LiveFeed.svelte
  // The Live Gravity Feed.
  //
  // 1. Opens a WebSocket to /ws on mount.
  // 2. On each `pulse:new` event, prepends a new card to the feed with
  //    a "Nile Flow" transition (slide-up + fade-in).
  // 3. Each card has interaction counters (echo, save) that animate via
  //    the `Odometer` component.
  // 4. Each card has a `SunCycleAura` showing the pulse's lifecycle phase.
  // ============================================================================

  import { onMount } from 'svelte';
  import SunCycleAura from './SunCycleAura.svelte';
  import Odometer from './Odometer.svelte';
  import { session } from '$stores/session';
  import { echoPulse, savePulse, repost } from '$lib/api/pulses';
  import { repost as repostApi } from '$lib/api/reposts';

  interface Pulse {
    pulse_id: string;
    author: string;
    ciphertext_b64: string;
    created_at: Date;
    echoes: number;
    saves: number;
    reposts: number;
    is_repost?: boolean;
    original_pulse_id?: string;
    reposted: boolean;
  }

  let pulses = $state<Pulse[]>([]);
  let ws: WebSocket | null = null;
  let connected = $state(false);
  let activeRepost = $state<Pulse | null>(null);
  let quoteText = $state('');

  const WS_URL =
    (import.meta.env.VITE_PUBLIC_WS_URL as string | undefined) ?? 'ws://localhost:8080/ws';

  onMount(() => {
    openSocket();
    return () => ws?.close();
  });

  function openSocket() {
    ws = new WebSocket(WS_URL);
    ws.onopen = () => (connected = true);
    ws.onclose = () => {
      connected = false;
      setTimeout(openSocket, 3000);
    };
    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data);
        if (msg.type === 'pulse:new') {
          prependPulse(msg);
        }
      } catch {
        // ignore non-JSON frames (heartbeats etc.)
      }
    };
  }

  function prependPulse(msg: any) {
    const pulse: Pulse = {
      pulse_id: msg.pulse_id,
      author: msg.author,
      ciphertext_b64: '',
      created_at: new Date(msg.created_at),
      echoes: 0,
      saves: 0,
      reposts: 0,
      is_repost: msg.is_repost ?? false,
      original_pulse_id: msg.original_pulse_id,
      reposted: false,
    };
    pulses = [pulse, ...pulses].slice(0, 100);
  }

  async function echo(p: Pulse) {
    p.echoes += 1;
    if ($session.userId) {
      try { await echoPulse($session.userId, p.pulse_id); } catch {}
    }
  }

  async function save(p: Pulse) {
    p.saves += 1;
    if ($session.userId) {
      try { await savePulse($session.userId, p.pulse_id); } catch {}
    }
  }

  function openRepostModal(p: Pulse) {
    activeRepost = p;
    quoteText = '';
  }

  function closeRepostModal() {
    activeRepost = null;
    quoteText = '';
  }

  async function doRepost() {
    if (!activeRepost || !$session.userId) return;
    try {
      // For plain repost (no quote), call without quote fields.
      await repostApi($session.userId, activeRepost.pulse_id);
      activeRepost.reposts += 1;
      activeRepost.reposted = true;
      closeRepostModal();
    } catch (e: any) {
      alert('فشل إعادة النشر: ' + (e?.message ?? 'خطأ'));
    }
  }

  async function doQuoteRepost() {
    if (!activeRepost || !$session.userId || !quoteText.trim()) return;
    try {
      // In production: encrypt quoteText, wrap key, call repost with quote fields.
      // For demo: send as plain repost (the quote is lost).
      await repostApi($session.userId, activeRepost.pulse_id);
      activeRepost.reposts += 1;
      activeRepost.reposted = true;
      closeRepostModal();
    } catch (e: any) {
      alert('فشل الاقتباس: ' + (e?.message ?? 'خطأ'));
    }
  }
</script>

<div class="max-w-2xl mx-auto py-8">
  <!-- Connection indicator -->
  <div class="flex items-center gap-2 mb-4 px-2">
    <div
      class="w-2 h-2 rounded-full {connected ? 'bg-egyptian-gold-400 aura-pulse' : 'bg-papyrus-400'}"
    ></div>
    <span class="text-xs text-papyrus-300">
      {connected ? 'الخلاصة حية' : 'بينقطع... هنوصّل تاني'}
    </span>
  </div>

  {#if pulses.length === 0}
    <div class="text-center py-16 text-papyrus-300">
      <p class="font-serif text-xl mb-2">النهر هادي</p>
      <p class="text-sm">أول نبضة حتظهر هنا تلقائياً</p>
    </div>
  {/if}

  {#each pulses as p (p.pulse_id)}
    <!-- Nile Flow transition: slide-up + fade-in -->
    <article
      class="mb-4 p-5 rounded-2xl bg-astral-blue-300/60 border border-egyptian-gold-400/20 sun-cycle-aura nile-flow-enter"
    >
      <header class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-3">
          <SunCycleAura createdAt={p.created_at} />
          <div>
            <div class="font-semibold text-papyrus-50">{p.author}</div>
            <div class="text-xs text-papyrus-300">
              {new Intl.RelativeTimeFormat('ar', { numeric: 'auto' }).format(
                Math.round((p.created_at.getTime() - Date.now()) / 60_000),
                'minute',
              )}
            </div>
          </div>
        </div>
      </header>

      <!-- The body is ciphertext; in production we'd decrypt client-side
           with the per-pulse ephemeral key. For the demo we render a placeholder. -->
      {#if p.is_repost}
        <div class="mb-2 text-xs text-papyrus-400 italic">
          🔁 إعادة نشر لنبضة {p.original_pulse_id?.slice(0, 8)}...
        </div>
      {/if}
      <div class="font-serif text-papyrus-100 leading-relaxed text-lg mb-4">
        🔒 نبضة مشفّرة — فك التشفير محلياً
      </div>

      <footer class="flex items-center gap-6 pt-3 border-t border-egyptian-gold-400/10">
        <button
          on:click={() => echo(p)}
          class="flex items-center gap-2 hover:text-egyptian-gold-300 transition"
        >
          <span class="text-xl">↻</span>
          <Odometer value={p.echoes} label="صدى" />
        </button>
        <button
          on:click={() => save(p)}
          class="flex items-center gap-2 hover:text-egyptian-gold-300 transition"
        >
          <span class="text-xl">⭐</span>
          <Odometer value={p.saves} label="حفظ" />
        </button>
        <button
          on:click={() => openRepostModal(p)}
          disabled={p.reposted}
          class="flex items-center gap-2 hover:text-egyptian-gold-300 transition disabled:opacity-30"
        >
          <span class="text-xl">🔁</span>
          <Odometer value={p.reposts} label="إعادة" />
        </button>
      </footer>
    </article>
  {/each}
</div>

<!-- Repost modal -->
{#if activeRepost}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4" on:click|self={closeRepostModal}>
    <div class="bg-astral-blue-400 border border-egyptian-gold-400/30 rounded-2xl p-6 max-w-md w-full">
      <h3 class="font-serif text-xl text-egyptian-gold-300 mb-4">إعادة نشر</h3>

      <div class="space-y-3">
        <button
          on:click={doRepost}
          class="w-full text-right p-3 rounded bg-astral-blue-500/50 hover:bg-astral-blue-500/80 transition text-papyrus-100"
        >
          <div class="font-serif">🔁 إعادة نشر بدون تعليق</div>
          <div class="text-xs text-papyrus-400">النبضة هتظهر لصدىك كما هي</div>
        </button>

        <div class="p-3 rounded bg-astral-blue-500/50">
          <div class="font-serif text-papyrus-100 mb-2">💭 اقتباس مع تعليق</div>
          <textarea
            bind:value={quoteText}
            rows="3"
            maxlength="280"
            placeholder="اكتب تعليقك على النبضة..."
            class="w-full bg-astral-blue-600 text-papyrus-50 p-2 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 resize-none text-sm"
          ></textarea>
          <button
            on:click={doQuoteRepost}
            disabled={!quoteText.trim()}
            class="mt-2 w-full py-2 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold disabled:opacity-50 hover:bg-egyptian-gold-300 transition"
          >
            اقتباس
          </button>
        </div>

        <button
          on:click={closeRepostModal}
          class="w-full py-2 text-papyrus-400 hover:text-papyrus-200 transition"
        >
          إلغاء
        </button>
      </div>
    </div>
  </div>
{/if}
