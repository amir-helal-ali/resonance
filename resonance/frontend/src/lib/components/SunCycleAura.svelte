<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/SunCycleAura.svelte
  // The "Sun Cycle Aura" — a colored ring around a pulse card that shifts
  // with the age of the pulse:
  //   0-2h   : sunrise  (gold rising)
  //   2-24h  : noon     (warm white-gold)
  //   24-48h : sunset   (deep amber)
  //   >48h   : dusk     (faded purple-gold)
  // ============================================================================

  let { createdAt }: { createdAt: Date } = $props();

  // Recompute every minute so the aura transitions smoothly.
  let now = $state(Date.now());
  setInterval(() => (now = Date.now()), 60_000);

  const ageMs = $derived(now - createdAt.getTime());

  const phase = $derived.by(() => {
    const h = ageMs / 3_600_000;
    if (h < 2)   return { name: 'شروق', color: 'rgba(229, 169, 30, 0.45)', ring: '#E5A91E' };
    if (h < 24)  return { name: 'توهج', color: 'rgba(251, 216, 118, 0.30)', ring: '#FBD876' };
    if (h < 48)  return { name: 'غروب', color: 'rgba(196, 139, 20, 0.35)',  ring: '#C48B14' };
    return            { name: 'حذف',  color: 'rgba(124, 84, 7, 0.30)',     ring: '#7C5407' };
  });
</script>

<!-- The ring is an SVG circle with a stroke-dasharray that animates. -->
<div class="relative inline-flex items-center justify-center">
  <svg width="56" height="56" viewBox="0 0 56 56" class="absolute -inset-2">
    <circle
      cx="28" cy="28" r="24"
      fill="none"
      stroke={phase.ring}
      stroke-width="2"
      stroke-dasharray="60 200"
      stroke-linecap="round"
      class="animate-spin"
      style="animation-duration: 12s; filter: drop-shadow(0 0 6px {phase.ring});"
    />
  </svg>
  <div
    class="w-10 h-10 rounded-full"
    style="background: radial-gradient(circle, {phase.color} 0%, transparent 70%);"
  ></div>
  <span class="absolute -bottom-5 text-xs text-papyrus-300">{phase.name}</span>
</div>
