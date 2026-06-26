<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/hashtag/[tag]/+page.svelte
  // Shows pulses with a specific hashtag.
  // ============================================================================

  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { session } from '$stores/session';
  import SunCycleAura from '$components/SunCycleAura.svelte';

  interface Pulse {
    pulse_id: string;
    author_id: string;
    ciphertext_b64: string;
    created_at: Date;
  }

  let pulses = $state<Pulse[]>([]);
  let loading = $state(true);

  const tag = $derived($page.params.tag);

  async function load() {
    if (!$session.userId || !tag) return;
    loading = true;
    try {
      // In production this would use signedFetch; for the demo we just simulate
      // an empty result (the discover endpoint requires a signature).
      pulses = [];
    } catch (e) {
      // silent
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (tag) load();
  });
</script>

<main class="max-w-2xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">#{tag}</h1>
  <p class="text-papyrus-300 text-sm mb-6">كل النبضات بالوسم ده</p>

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if pulses.length === 0}
    <div class="text-center py-12">
      <div class="text-5xl mb-3">#</div>
      <p class="text-papyrus-200">مفيش نبضات بالوسم ده</p>
    </div>
  {:else}
    <div class="space-y-3">
      {#each pulses as p (p.pulse_id)}
        <article class="p-5 rounded-2xl bg-astral-blue-300/60 border border-egyptian-gold-400/20 sun-cycle-aura">
          <header class="flex items-center gap-3 mb-3">
            <SunCycleAura createdAt={p.created_at} />
            <div>
              <span class="font-mono text-xs text-papyrus-400">{p.author_id.slice(0, 8)}...</span>
            </div>
          </header>
          <div class="font-serif text-papyrus-100 p-3 rounded bg-astral-blue-500/40 text-center text-sm">
            🔒 نبضة مشفّرة
          </div>
        </article>
      {/each}
    </div>
  {/if}
</main>
