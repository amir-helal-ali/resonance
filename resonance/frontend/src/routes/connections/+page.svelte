<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/connections/+page.svelte
  // Manage your resonances (صدى) + Co-Resonance friend suggestions.
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import {
    listConnections,
    suggestConnections,
    unsync,
    syncConnection,
    type ConnectionOut,
    type SuggestionEntry,
  } from '$lib/api/connections';

  let connections = $state<ConnectionOut[]>([]);
  let suggestions = $state<SuggestionEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      [connections, suggestions] = await Promise.all([
        listConnections($session.userId),
        suggestConnections($session.userId),
      ]);
    } catch (e: any) {
      error = e?.message ?? 'فشل تحميل الصدى';
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function handleUnsync(target: string) {
    if (!$session.userId) return;
    try {
      await unsync($session.userId, target);
      connections = connections.filter((c) => c.target_user_id !== target);
    } catch (e: any) {
      error = e?.message ?? 'فشل فك الصدى';
    }
  }

  async function handleSync(target: string) {
    if (!$session.userId) return;
    try {
      await syncConnection($session.userId, target, 5);
      await refresh();
    } catch (e: any) {
      error = e?.message ?? 'فشل الصدى';
    }
  }

  function scoreColor(score: number): string {
    if (score >= 80) return 'text-egyptian-gold-300';
    if (score >= 50) return 'text-egyptian-gold-400';
    if (score >= 20) return 'text-papyrus-200';
    return 'text-papyrus-400';
  }

  function scoreBarWidth(score: number): string {
    return `${Math.min(100, score)}%`;
  }

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const days = Math.floor(diff / 86_400_000);
    const hours = Math.floor(diff / 3_600_000);
    if (days > 0) return `من ${days} يوم`;
    if (hours > 0) return `من ${hours} ساعة`;
    return 'دلوقتي';
  }
</script>

<main class="max-w-3xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">صدى</h1>
  <p class="text-papyrus-300 text-sm mb-6">مزامناتك الحية — بتبخر كل 7 أيام من غير تفاعل</p>

  {#if error}
    <div class="mb-4 p-3 rounded bg-red-900/40 border border-red-500 text-red-200 text-sm">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else}
    <!-- Active resonances -->
    <section class="mb-10">
      <h2 class="text-xs uppercase tracking-wider text-papyrus-300 mb-3">صدىك النشط</h2>
      {#if connections.length === 0}
        <div class="text-papyrus-400 text-sm p-4 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10">
          لسه معندكش صدى. ابدأ بمتابعة حد من الاقتراحات تحت.
        </div>
      {:else}
        <div class="space-y-2">
          {#each connections as c}
            <div class="flex items-center gap-4 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10">
              <div class="flex-1">
                <div class="flex items-center justify-between mb-1">
                  <span class="text-papyrus-100 font-mono text-sm">
                    {c.target_user_id.slice(0, 8)}...
                  </span>
                  <span class="text-xs text-papyrus-400">{timeAgo(c.last_interaction_at)}</span>
                </div>
                <div class="h-2 rounded-full bg-astral-blue-500 overflow-hidden">
                  <div
                    class="h-full bg-gradient-to-r from-egyptian-gold-500 to-egyptian-gold-300 transition-all"
                    style="width: {scoreBarWidth(c.resonance_score)};"
                  ></div>
                </div>
              </div>
              <span class="font-mono text-sm {scoreColor(c.resonance_score)} w-12 text-right">
                {c.resonance_score}%
              </span>
              <button
                on:click={() => handleUnsync(c.target_user_id)}
                class="text-xs text-papyrus-400 hover:text-red-300 transition px-2 py-1 rounded hover:bg-red-900/30"
              >
                فك
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Co-Resonance suggestions -->
    <section>
      <h2 class="text-xs uppercase tracking-wider text-papyrus-300 mb-3">اقتراحات صدى</h2>
      <p class="text-xs text-papyrus-400 mb-3">
        مبنية على Co-Resonance (Jaccard similarity) — ناس بيصدّوا على نفس الناس اللي بتصدّ عليهم.
      </p>
      {#if suggestions.length === 0}
        <div class="text-papyrus-400 text-sm p-4 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10">
          مفيش اقتراحات لسه. صدّ على ناس أكتر عشان نقدر نقترح.
        </div>
      {:else}
        <div class="space-y-2">
          {#each suggestions as s}
            <div class="flex items-center gap-4 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10">
              <div class="flex-1">
                <div class="text-papyrus-100 font-serif">{s.username}</div>
                <div class="text-xs text-papyrus-400">
                  Jaccard: {(s.jaccard * 100).toFixed(1)}%
                </div>
              </div>
              <button
                on:click={() => handleSync(s.user_id)}
                class="px-4 py-1.5 rounded bg-egyptian-gold-400/20 text-egyptian-gold-300 text-sm hover:bg-egyptian-gold-400 hover:text-astral-blue-700 transition"
              >
                صدّ
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</main>
