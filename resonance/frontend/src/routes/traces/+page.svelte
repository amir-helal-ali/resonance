<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/traces/+page.svelte
  // The "آثار عابرة" (Passing Traces) page.
  //
  // Shows recent profile visits to the requester. Anonymous visits show as
  // "زائر مجهول"; named visits show the visitor's username.
  // All traces auto-evaporate after 7 days.
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import { listMyTraces, type TraceOut } from '$lib/api/presence';

  let traces = $state<TraceOut[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      traces = await listMyTraces($session.userId);
    } catch (e: any) {
      error = e?.message ?? 'فشل تحميل الآثار';
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const mins = Math.floor(diff / 60_000);
    const hours = Math.floor(diff / 3_600_000);
    const days = Math.floor(diff / 86_400_000);
    if (days > 0) return `من ${days} يوم`;
    if (hours > 0) return `من ${hours} ساعة`;
    if (mins > 0) return `من ${mins} دقيقة`;
    return 'دلوقتي';
  }

  function timeLeft(iso: string): string {
    const diff = new Date(iso).getTime() - Date.now();
    const hours = Math.floor(diff / 3_600_000);
    const days = Math.floor(diff / 86_400_000);
    if (days > 0) return `هيفضل ${days} يوم`;
    if (hours > 0) return `هيفضل ${hours} ساعة`;
    return 'هيتشاف دلوقتي';
  }
</script>

<main class="max-w-3xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">آثار عابرة</h1>
  <p class="text-papyrus-300 text-sm mb-6">
    ناس زاروا ملفك. بيتشالوا آلياً بعد 7 أيام.
  </p>

  {#if error}
    <div class="mb-4 p-3 rounded bg-red-900/40 border border-red-500 text-red-200 text-sm">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if traces.length === 0}
    <div class="text-center py-12">
      <div class="text-5xl mb-3">🌅</div>
      <p class="text-papyrus-200">مفيش آثار لسه</p>
      <p class="text-papyrus-400 text-sm mt-1">لما حد يزور ملفك، هتلاقي أثره هنا</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each traces as t}
        <div class="flex items-center gap-3 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10">
          <div class="w-10 h-10 rounded-full bg-egyptian-gold-400/20 flex items-center justify-center text-egyptian-gold-300">
            {#if t.kind === 'named'}
              👤
            {:else}
              🌫️
            {/if}
          </div>
          <div class="flex-1">
            <div class="text-papyrus-100 text-sm">
              {#if t.kind === 'named' && t.visitor_username}
                <span class="font-serif">{t.visitor_username}</span>
                <span class="text-papyrus-400"> زارك</span>
              {:else}
                زائر مجهول زارك
              {/if}
            </div>
            <div class="text-xs text-papyrus-400">
              {timeAgo(t.created_at)} · {timeLeft(t.expires_at)}
            </div>
          </div>
          {#if t.kind === 'anonymous'}
            <span class="text-xs text-papyrus-500 px-2 py-1 rounded bg-astral-blue-500/50">
              مجهول
            </span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</main>
