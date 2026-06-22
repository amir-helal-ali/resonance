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

  interface Pulse {
    pulse_id: string;
    author: string;
    ciphertext_b64: string;
    created_at: Date;
    echoes: number;
    saves: number;
  }

  let pulses = $state<Pulse[]>([]);
  let ws: WebSocket | null = null;
  let connected = $state(false);

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
      // Reconnect after 3s.
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
      ciphertext_b64: '', // not present in the WS event; we'd fetch on demand
      created_at: new Date(msg.created_at),
      echoes: 0,
      saves: 0,
    };
    pulses = [pulse, ...pulses].slice(0, 100);
  }

  function echo(p: Pulse) {
    p.echoes += 1;
    // In production this would call /api/pulses/:id/echo with a signed request.
  }

  function save(p: Pulse) {
    p.saves += 1;
    // In production: signed POST /api/pulses/:id/save
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
      </footer>
    </article>
  {/each}
</div>
