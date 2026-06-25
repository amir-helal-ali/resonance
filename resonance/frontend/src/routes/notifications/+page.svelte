<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/notifications/+page.svelte
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import {
    listNotifications,
    markAllRead,
    markRead,
    type NotificationOut,
  } from '$lib/api/notifications';

  let notifications = $state<NotificationOut[]>([]);
  let loading = $state(true);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      notifications = await listNotifications($session.userId, { limit: 50 });
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function handleMarkAll() {
    if (!$session.userId) return;
    await markAllRead($session.userId);
    notifications = notifications.map((n) => ({ ...n, read_at: new Date().toISOString() }));
  }

  async function handleClick(n: NotificationOut) {
    if (!n.read_at && $session.userId) {
      await markRead($session.userId, n.id);
      n.read_at = new Date().toISOString();
    }
  }

  function kindIcon(kind: string): string {
    return {
      echo: '↻',
      save: '⭐',
      comment: '💬',
      sync: '🔗',
      resonance: '🌊',
      mention: '@',
      jury_summon: '⚖️',
      jury_verdict: '⚖️',
      goal_lit: '🕯️',
      candle: '🕯️',
      trace: '👁️',
    }[kind] ?? '🔔';
  }

  function kindLabel(kind: string): string {
    return {
      echo: 'صدّ على نبضتك',
      save: 'حفظ نبضتك',
      comment: 'علّق على نبضتك',
      sync: 'صدّ عليك',
      resonance: 'الصدى بينكم اتخطّى 50%',
      mention: 'ذكرك',
      jury_summon: 'تم استدعائك لمحكمة',
      jury_verdict: 'المحكمة أصدرت حكمها',
      goal_lit: 'هدفك اتنوّر! 🎉',
      candle: 'حد شال شمعة لهدفك',
      trace: 'حد زار ملفك',
    }[kind] ?? 'إشعار';
  }

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const mins = Math.floor(diff / 60_000);
    const hours = Math.floor(diff / 3_600_000);
    const days = Math.floor(diff / 86_400_000);
    if (days > 0) return `من ${days} يوم`;
    if (hours > 0) return `من ${hours} ساعة`;
    if (mins > 0) return `من ${mins} دقيقة`;
    return 'دلوقتي';
  }
</script>

<main class="max-w-2xl mx-auto py-8 px-4">
  <div class="flex items-center justify-between mb-6">
    <h1 class="font-serif text-3xl text-egyptian-gold-400">الإشعارات</h1>
    {#if notifications.some((n) => !n.read_at)}
      <button
        on:click={handleMarkAll}
        class="text-sm text-egyptian-gold-300 hover:text-egyptian-gold-400 transition"
      >
        علّم الكل كمقروء
      </button>
    {/if}
  </div>

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if notifications.length === 0}
    <div class="text-center py-12">
      <div class="text-5xl mb-3">🔔</div>
      <p class="text-papyrus-200">مفيش إشعارات</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each notifications as n (n.id)}
        <button
          on:click={() => handleClick(n)}
          class="w-full text-right flex items-start gap-3 p-3 rounded bg-astral-blue-400/40 border border-egyptian-gold-400/10 hover:bg-astral-blue-400/60 transition {!n.read_at ? 'border-egyptian-gold-400/30' : ''}"
        >
          <span class="text-2xl flex-shrink-0">{kindIcon(n.kind)}</span>
          <div class="flex-1 min-w-0">
            <div class="text-sm text-papyrus-100">
              {#if n.actor_username}
                <span class="font-serif text-egyptian-gold-300">{n.actor_username}</span>
              {/if}
              {kindLabel(n.kind)}
            </div>
            <div class="text-xs text-papyrus-400 mt-0.5">{timeAgo(n.created_at)}</div>
          </div>
          {#if !n.read_at}
            <span class="w-2 h-2 rounded-full bg-egyptian-gold-400 flex-shrink-0 mt-1.5"></span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</main>
