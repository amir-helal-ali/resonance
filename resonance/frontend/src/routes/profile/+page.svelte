<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/profile/+page.svelte
  // Public profile page. Looks up the user by username (from ?u= query param)
  // or falls back to a demo profile if no param.
  // ============================================================================

  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import LivingProfile from '$components/LivingProfile.svelte';
  import { session } from '$stores/session';
  import {
    lookupByUsername,
    type UserProfileOut,
  } from '$lib/api/discover';
  import { listPresence, pulsePresence, type PresenceEntry } from '$lib/api/presence';
  import { listGoals, type GoalOut } from '$lib/api/goals';

  let profile = $state<UserProfileOut | null>(null);
  let presence = $state<PresenceEntry[]>([]);
  let goals = $state<GoalOut[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load(username: string) {
    if (!$session.userId) {
      loading = false;
      return;
    }
    loading = true;
    error = null;
    try {
      // 1. Lookup the user by username.
      profile = await lookupByUsername($session.userId, username);

      // 2. Record our visit; this may set the aura if our resonance > 50%.
      try {
        await pulsePresence($session.userId, profile.user_id, true);
      } catch {
        // ignore — profile still renders
      }

      // 3. Fetch presence + goals in parallel.
      [presence, goals] = await Promise.all([
        listPresence($session.userId, profile.user_id).catch(() => [] as PresenceEntry[]),
        listGoals($session.userId, profile.user_id).catch(() => [] as GoalOut[]),
      ]);
    } catch (e: any) {
      error = e?.message ?? 'فشل تحميل الملف الشخصي';
    } finally {
      loading = false;
    }
  }

  // React to ?u= query param changes.
  $effect(() => {
    const username = $page.url.searchParams.get('u');
    if (username && $session.userId) {
      load(username);
    } else {
      loading = false;
    }
  });

  function badgeIcon(badge: string): string {
    return { verified: '✓', creator: '🎨', expert: '🧠', founder: '🌟' }[badge] ?? '';
  }
</script>

<main class="min-h-screen py-8 px-4">
  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if error}
    <div class="max-w-2xl mx-auto p-4 rounded bg-red-900/40 border border-red-500 text-red-200 text-sm">
      {error}
    </div>
  {:else if profile}
    <div class="max-w-2xl mx-auto mb-4 flex items-center gap-2 text-sm text-egyptian-gold-300">
      {#each profile.badges as b}
        <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-egyptian-gold-400/20 text-xs">
          {badgeIcon(b)} {b}
        </span>
      {/each}
    </div>
    <LivingProfile
      username={profile.username}
      imprint={profile.imprint || 'بصمة لسه مكتوبة.'}
      horizon={profile.horizon || 'أُفق لسه مرسوم.'}
      createdAt={new Date(profile.created_at)}
      {presence}
      goals={goals.map((g) => ({
        id: g.id,
        title: g.title,
        current: g.current,
        target: g.target,
      }))}
    />
  {:else}
    <div class="max-w-2xl mx-auto p-4 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10 text-center text-papyrus-300">
      <p>ادخل على ملف أي حد عبر البحث أو من <a href="/feed" class="text-egyptian-gold-300 underline">الخلاصة</a>.</p>
    </div>
  {/if}
</main>
