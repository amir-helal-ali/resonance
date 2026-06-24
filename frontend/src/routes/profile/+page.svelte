<script lang="ts">
  import { onMount } from 'svelte';
  import LivingProfile from '$components/LivingProfile.svelte';
  import { session } from '$stores/session';
  import { listPresence, pulsePresence, type PresenceEntry } from '$lib/api/presence';

  let presence = $state<PresenceEntry[]>([]);
  let loading = $state(true);

  // Hardcoded profile for demo. In production we'd fetch by route param.
  const PROFILE = {
    username: 'nefertiti',
    imprint:
      'من وادي النيل، أحمل صوت أجدادي. أكتب لأذكر، وأخفض صوتي حين يعلو الضجيج.',
    horizon:
      'أن أبني مكتبة رقمية تُترجم إليها كل لغة مهددة بالانقراض. عشان كل لغة فيها ذاكرة شعب.',
    createdAt: new Date(Date.now() - 1000 * 60 * 60 * 18), // 18h ago → "noon" phase
    goals: [
      { id: 'g1', title: 'ترجمة 50 قصيدة نوبية', current: 23, target: 50 },
      { id: 'g2', title: 'توثيق 100 موقع أثري',  current: 8,  target: 100 },
    ],
  };

  const TARGET_USER_ID = '00000000-0000-0000-0000-000000000001';

  onMount(async () => {
    if (!session.unlocked) {
      loading = false;
      return;
    }
    try {
      // Record our visit; this may set the aura if our resonance > 50%.
      await pulsePresence(session.userId!, TARGET_USER_ID, true);
      // Fetch the current presence list.
      presence = await listPresence(session.userId!, TARGET_USER_ID);
    } catch (e) {
      // ignore — the profile still renders
    } finally {
      loading = false;
    }
  });
</script>

<main class="min-h-screen py-8">
  {#if loading}
    <div class="text-center text-papyrus-300">بيتحمّل...</div>
  {:else}
    <LivingProfile
      username={PROFILE.username}
      imprint={PROFILE.imprint}
      horizon={PROFILE.horizon}
      createdAt={PROFILE.createdAt}
      {presence}
      goals={PROFILE.goals}
    />
  {/if}
</main>
