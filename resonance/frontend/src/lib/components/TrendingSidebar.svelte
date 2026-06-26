<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/TrendingSidebar.svelte
  // Right sidebar showing trending hashtags + suggested users.
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

  function badgeIcon(badge: string): string {
    return { verified: '✓', creator: '🎨', expert: '🧠', founder: '🌟' }[badge] ?? '•';
  }
</script>

<aside class="space-y-4">
  <!-- Trending hashtags -->
  <div class="p-4 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
    <h3 class="text-xs uppercase tracking-wider text-papyrus-300 mb-3">الوسوم الرائجة</h3>
    {#if loading}
      <div class="text-sm text-papyrus-400">بيتحمّل...</div>
    {:else if trending.length === 0}
      <div class="text-sm text-papyrus-400">مفيش وسوم رائجة دلوقتي</div>
    {:else}
      <div class="space-y-2">
        {#each trending as t}
          <a
            href="/search?tag={t.tag}"
            class="block p-2 rounded hover:bg-astral-blue-400/50 transition"
          >
            <div class="font-serif text-papyrus-100">#{t.tag}</div>
            <div class="text-xs text-papyrus-400">
              {t.pulse_count} نبضة · {t.unique_authors} صانع
            </div>
          </a>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Suggested users -->
  <div class="p-4 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
    <h3 class="text-xs uppercase tracking-wider text-papyrus-300 mb-3">مقترحات ليك</h3>
    {#if loading}
      <div class="text-sm text-papyrus-400">بيتحمّل...</div>
    {:else if suggested.length === 0}
      <div class="text-sm text-papyrus-400">مفيش مقترحات دلوقتي</div>
    {:else}
      <div class="space-y-2">
        {#each suggested as s}
          <a
            href="/profile?u={s.username}"
            class="flex items-center gap-2 p-2 rounded hover:bg-astral-blue-400/50 transition"
          >
            <div class="w-8 h-8 rounded-full bg-egyptian-gold-400/20 flex items-center justify-center text-xs text-egyptian-gold-300">
              👤
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1">
                <span class="font-serif text-papyrus-100 text-sm truncate">{s.username}</span>
                {#each s.badges as b}
                  <span class="text-xs text-egyptian-gold-300" title={b}>{badgeIcon(b)}</span>
                {/each}
              </div>
              {#if s.imprint_preview}
                <div class="text-xs text-papyrus-400 truncate">{s.imprint_preview}</div>
              {/if}
            </div>
          </a>
        {/each}
      </div>
    {/if}
  </div>
</aside>
