<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/LivingProfile.svelte
  // The Living Profile & Manifesto.
  //
  // Layout:
  //   ┌──────────────────────────────────────────────────────┐
  //   │  [Avatar + Sun Cycle Aura]   username                │
  //   │                                                       │
  //   │  "بصمتي" (Amiri serif, gold)                          │
  //   │  <imprint text>                                       │
  //   │                                                       │
  //   │  "أُفقي" (Amiri serif, gold)                            │
  //   │  <horizon text>                                       │
  //   │                                                       │
  //   │  [Nile Presence Bar]                                  │
  //   │                                                       │
  //   │  [شموع الدعم — Goal Candles]                            │
  //   │  🕯️ 23 / 50    🕯️ 8 / 100                              │
  //   └──────────────────────────────────────────────────────┘
  // ============================================================================

  import SunCycleAura from './SunCycleAura.svelte';
  import NilePresenceBar from './NilePresenceBar.svelte';
  import type { PresenceEntry } from '$lib/api/presence';

  interface GoalCandle {
    id: string;
    title: string;
    current: number;
    target: number;
  }

  let {
    username,
    imprint,
    horizon,
    createdAt = new Date(),
    presence = [] as PresenceEntry[],
    goals = [] as GoalCandle[],
  }: {
    username: string;
    imprint: string;
    horizon: string;
    createdAt?: Date;
    presence?: PresenceEntry[];
    goals?: GoalCandle[];
  } = $props();

  // Light a candle when clicked — purely visual; in production this would
  // call /api/goals/:id/support with a signed request.
  function lightCandle(g: GoalCandle) {
    g.current = Math.min(g.current + 1, g.target);
  }
</script>

<div class="max-w-2xl mx-auto p-6 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20 sun-cycle-aura">
  <!-- Header: avatar + aura + username -->
  <div class="flex items-center gap-4 mb-6">
    <div class="relative">
      <SunCycleAura {createdAt} />
    </div>
    <div>
      <h1 class="font-serif text-3xl text-egyptian-gold-400">{username}</h1>
      <p class="text-xs text-papyrus-300 mt-1">بينبض الآن</p>
    </div>
  </div>

  <!-- بصمتي (Imprint) -->
  <section class="mb-6">
    <h2 class="font-serif text-xl text-egyptian-gold-300 mb-2">بصمتي</h2>
    <p class="font-serif text-papyrus-100 leading-relaxed text-lg">
      {imprint}
    </p>
  </section>

  <!-- أُفقي (Horizon) -->
  <section class="mb-6">
    <h2 class="font-serif text-xl text-egyptian-gold-300 mb-2">أُفقي</h2>
    <p class="font-serif text-papyrus-100 leading-relaxed text-lg">
      {horizon}
    </p>
  </section>

  <!-- Nile Presence Bar -->
  <section class="mb-6">
    <h3 class="text-xs uppercase tracking-wider text-papyrus-300 mb-2">حضور النيل</h3>
    <NilePresenceBar entries={presence} />
  </section>

  <!-- شموع الدعم (Goal Candles) -->
  {#if goals.length > 0}
    <section>
      <h3 class="text-xs uppercase tracking-wider text-papyrus-300 mb-3">شموع الدعم</h3>
      <div class="space-y-3">
        {#each goals as g}
          <div class="flex items-center gap-3">
            <button
              on:click={() => lightCandle(g)}
              class="text-2xl hover:scale-110 transition"
              title="أشعل شمعة"
            >🕯️</button>
            <div class="flex-1">
              <div class="flex justify-between text-sm">
                <span class="text-papyrus-100">{g.title}</span>
                <span class="text-egyptian-gold-300">{g.current} / {g.target}</span>
              </div>
              <div class="h-2 rounded-full bg-astral-blue-500 overflow-hidden mt-1">
                <div
                  class="h-full bg-gradient-to-r from-egyptian-gold-500 to-egyptian-gold-300 transition-all"
                  style="width: {Math.min(100, (g.current / g.target) * 100)}%;"
                ></div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>
