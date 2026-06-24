<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/NilePresenceBar.svelte
  // The "Nile Presence Bar" — a horizontal indicator showing how many users
  // are currently "Pulsing Now" on this profile.
  //
  // Each pulsing user is represented as a golden dot flowing along the bar
  // like debris on the Nile.
  // ============================================================================

  import type { PresenceEntry } from '$lib/api/presence';

  let { entries }: { entries: PresenceEntry[] } = $props();

  // Generate a stable pseudo-random offset for each entry, so dots don't
  // all stack on top of each other.
  function offsetFor(id: string): number {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
    return (h % 100) / 100;
  }
</script>

<div class="relative h-8 w-full overflow-hidden rounded-full bg-astral-blue-500/40 border border-egyptian-gold-400/20">
  <!-- The Nile wave background -->
  <div
    class="absolute inset-0 opacity-30"
    style="background: linear-gradient(90deg,
      transparent 0%,
      rgba(229,169,30,0.4) 30%,
      rgba(251,216,118,0.6) 50%,
      rgba(229,169,30,0.4) 70%,
      transparent 100%);
      background-size: 200% 100%;
      animation: sun-cycle 8s linear infinite;"
  ></div>

  {#each entries as entry}
    <div
      class="absolute top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-egyptian-gold-300 aura-pulse"
      style="left: {offsetFor(entry.user_id) * 90 + 2}%;"
      title={entry.username}
    ></div>
  {/each}

  {#if entries.length === 0}
    <div class="absolute inset-0 flex items-center justify-center text-xs text-papyrus-400">
      مفيش حد بينبض دلوقتي
    </div>
  {:else}
    <div class="absolute inset-0 flex items-center justify-center text-xs text-papyrus-200 pointer-events-none">
      {entries.length} بينبضوا الآن
    </div>
  {/if}
</div>
