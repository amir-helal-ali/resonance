<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/saved/+page.svelte
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import { listSaved, type SavedPulseOut } from '$lib/api/settings';

  let saved = $state<SavedPulseOut[]>([]);
  let loading = $state(true);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      saved = await listSaved($session.userId);
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const days = Math.floor(diff / 86_400_000);
    const hours = Math.floor(diff / 3_600_000);
    if (days > 0) return `من ${days} يوم`;
    if (hours > 0) return `من ${hours} ساعة`;
    return 'دلوقتي';
  }
</script>

<main class="max-w-2xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">المحفوظات</h1>
  <p class="text-papyrus-300 text-sm mb-6">نبضات اتحفظت ليك — ما حدش غيرك بيشوفها</p>

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if saved.length === 0}
    <div class="text-center py-12">
      <div class="text-5xl mb-3">⭐</div>
      <p class="text-papyrus-200">مفيش نبضات محفوظة</p>
      <p class="text-papyrus-400 text-sm mt-1">اضغط ⭐ على أي نبضة عشان تحفظها هنا</p>
    </div>
  {:else}
    <div class="space-y-3">
      {#each saved as s}
        <article class="p-4 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/10">
          <div class="flex items-center justify-between mb-2">
            <span class="font-mono text-xs text-papyrus-400">من {s.author_id.slice(0, 8)}...</span>
            <span class="text-xs text-papyrus-400">اتحفظ {timeAgo(s.saved_at)}</span>
          </div>
          <div class="font-serif text-papyrus-100 p-3 rounded bg-astral-blue-500/40 text-center text-sm mb-2">
            🔒 نبضة مشفّرة
          </div>
          {#if s.personal_note}
            <div class="text-sm text-egyptian-gold-300 italic">ملاحظتك: {s.personal_note}</div>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</main>
