<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/search/+page.svelte
  // ============================================================================

  import { session } from '$stores/session';
  import { search, type SearchResult } from '$lib/api/search';

  let query = $state('');
  let results = $state<SearchResult[]>([]);
  let loading = $state(false);
  let searched = $state(false);
  let kind = $state<'all' | 'users' | 'hashtags'>('all');

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  async function doSearch() {
    if (!$session.userId || !query.trim()) {
      results = [];
      searched = false;
      return;
    }
    loading = true;
    searched = true;
    try {
      results = await search($session.userId, query, kind, 30);
    } catch (e) {
      console.error(e);
      results = [];
    } finally {
      loading = false;
    }
  }

  function onInput() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(doSearch, 350);
  }
</script>

<main class="max-w-2xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-6">البحث</h1>

  <input
    bind:value={query}
    on:input={onInput}
    placeholder="دوّر على ناس أو وسوم..."
    class="w-full bg-astral-blue-500 text-papyrus-50 px-4 py-3 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 mb-3"
  />

  <div class="flex gap-2 mb-6">
    {#each ['all', 'users', 'hashtags'] as k}
      <button
        on:click={() => { kind = k as any; doSearch(); }}
        class="px-3 py-1 rounded text-sm {kind === k ? 'bg-egyptian-gold-400 text-astral-blue-700' : 'bg-astral-blue-500/50 text-papyrus-300'} transition"
      >
        {k === 'all' ? 'الكل' : k === 'users' ? 'ناس' : 'وسوم'}
      </button>
    {/each}
  </div>

  {#if loading}
    <div class="text-center py-8 text-papyrus-300">بي دوّر...</div>
  {:else if searched && results.length === 0}
    <div class="text-center py-8 text-papyrus-400">
      مفيش نتائج لـ "{query}"
    </div>
  {:else if results.length > 0}
    <div class="space-y-2">
      {#each results as r}
        {#if r.type === 'user'}
          <a
            href="/profile?u={r.user_id}"
            class="flex items-center gap-3 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10 hover:bg-astral-blue-400/60 transition"
          >
            <div class="w-10 h-10 rounded-full bg-egyptian-gold-400/20 flex items-center justify-center text-egyptian-gold-300">
              👤
            </div>
            <div class="flex-1">
              <div class="font-serif text-papyrus-100">{r.username}</div>
              {#if r.imprint_preview}
                <div class="text-xs text-papyrus-400 truncate">{r.imprint_preview}</div>
              {/if}
            </div>
          </a>
        {:else}
          <a
            href="/search?tag={r.tag}"
            class="flex items-center gap-3 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10 hover:bg-astral-blue-400/60 transition"
          >
            <div class="w-10 h-10 rounded-full bg-egyptian-gold-400/20 flex items-center justify-center text-egyptian-gold-300">
              #
            </div>
            <div class="flex-1">
              <div class="font-serif text-papyrus-100">#{r.tag}</div>
              <div class="text-xs text-papyrus-400">{r.pulse_count} نبضة</div>
            </div>
          </a>
        {/if}
      {/each}
    </div>
  {:else}
    <div class="text-center py-12 text-papyrus-400">
      اكتب اسم أو وسم في الأعلى عشان تدوّر
    </div>
  {/if}
</main>
