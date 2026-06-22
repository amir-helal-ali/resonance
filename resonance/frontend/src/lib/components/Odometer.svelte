<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/Odometer.svelte
  // An interaction counter that animates like a speedometer (odometer).
  //
  // When the value changes, we animate from the old value to the new value
  // over ~400ms with an ease-out curve.
  // ============================================================================

  let { value = 0, label = '' }: { value?: number; label?: string } = $props();

  let display = $state(value);
  let rafId: number | null = null;
  let fromValue = value;
  let toValue = value;
  let startTime = 0;

  // When `value` changes, kick off an animation.
  $effect(() => {
    if (value === toValue) return;
    fromValue = display;
    toValue = value;
    startTime = performance.now();
    if (rafId !== null) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(animate);
  });

  function animate(t: number) {
    const elapsed = t - startTime;
    const duration = 400;
    const k = Math.min(1, elapsed / duration);
    // ease-out cubic
    const eased = 1 - Math.pow(1 - k, 3);
    display = Math.round(fromValue + (toValue - fromValue) * eased);
    if (k < 1) {
      rafId = requestAnimationFrame(animate);
    } else {
      rafId = null;
    }
  }
</script>

<div class="inline-flex flex-col items-center">
  <span class="font-mono text-2xl tabular-nums text-egyptian-gold-300">
    {display.toLocaleString('ar-EG')}
  </span>
  {#if label}
    <span class="text-xs text-papyrus-300 mt-0.5">{label}</span>
  {/if}
</div>
