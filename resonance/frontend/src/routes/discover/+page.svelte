<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/discover/+page.svelte
  // Discover page: trending hashtags + suggested users side by side.
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import {
    getTrending,
    getSuggestedUsers,
    type TrendingHashtag,
    type SuggestedUser,
  } from '$lib/api/discover';

  let trending = $state<TrendingHashtag[]>([]);
  let suggested = $state<SuggestedUser[]>([]);
  let loading = $state(true);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      [trending, suggested] = await Promise.all([
        getTrending($session.userId),
        getSuggestedUsers($session.userId),
      ]);
    } catch (e) {
      // silent
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  function badgeIcon(b: string): string {
    return { verified: '✓', creator: '🎨', expert: '🧠', founder: '🌟' }[b] ?? '';
  }
</script>

<main class="max-w-4xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">استكشف صدى</h1>
  <p class="text-papyrus-300 text-sm mb-8">اللي بيتكلم عليه الناس دلوقتي</p>

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else}
    <div class="grid md:grid-cols-2 gap-6">
      <!-- Trending -->
      <section class="p-5 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
        <h2 class="font-serif text-xl text-egyptian-gold-300 mb-4">🔥 الوسوم الرائجة</h2>
        {#if trending.length === 0}
          <p class="text-sm text-papyrus-400">مفيش وسوم رائجة دلوقتي</p>
        {:else}
          <div class="space-y-2">
            {#each trending as t, i}
              <a
                href="/hashtag/{t.tag}"
                class="flex items-center gap-3 p-3 rounded bg-astral-blue-400/50 hover:bg-astral-blue-400/80 transition"
              >
                <span class="font-mono text-2xl text-egyptian-gold-400/40 w-8">{i + 1}</span>
                <div class="flex-1">
                  <div class="font-serif text-papyrus-100">#{t.tag}</div>
                  <div class="text-xs text-papyrus-400">
                    {t.pulse_count} نبضة · {t.unique_authors} صانع
                  </div>
                </div>
              </a>
            {/each}
          </div>
        {/if}
      </section>

      <!-- Suggested users -->
      <section class="p-5 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
        <h2 class="font-serif text-xl text-egyptian-gold-300 mb-4">✨ مقترحات ليك</h2>
        {#if suggested.length === 0}
          <p class="text-sm text-papyrus-400">مفيش مقترحات دلوقتي</p>
        {:else}
          <div class="space-y-2">
            {#each suggested as s}
              <a
                href="/profile?u={s.username}"
                class="flex items-center gap-3 p-3 rounded bg-astral-blue-400/50 hover:bg-astral-blue-400/80 transition"
              >
                <div class="w-10 h-10 rounded-full bg-egyptian-gold-400/20 flex items-center justify-center text-egyptian-gold-300">
                  👤
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-1">
                    <span class="font-serif text-papyrus-100">{s.username}</span>
                    {#each s.badges as b}
                      <span class="text-xs text-egyptian-gold-300" title={b}>{badgeIcon(b)}</span>
                    {/each}
                  </div>
                  {#if s.imprint_preview}
                    <div class="text-xs text-papyrus-400 truncate">{s.imprint_preview}</div>
                  {/if}
                  {#if s.jaccard > 0}
                    <div class="text-xs text-egyptian-gold-400/60">
                      {(s.jaccard * 100).toFixed(0)}% تشابه في الصدى
                    </div>
                  {/if}
                </div>
              </a>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</main>
