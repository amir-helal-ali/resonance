<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/Navbar.svelte
  // Top navigation with live unread-notification badge.
  // ============================================================================

  import { onMount, onDestroy } from 'svelte';
  import { session, clearSession } from '$stores/session';
  import { unreadCount } from '$lib/api/notifications';
  import { connectPersonalWS, disconnectPersonalWS, wsConnected } from '$stores/personal_ws';

  let unread = $state(0);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function refreshUnread() {
    if (!$session.userId) return;
    try {
      unread = await unreadCount($session.userId);
    } catch {
      // ignore
    }
  }

  onMount(() => {
    if ($session.userId) {
      connectPersonalWS($session.userId);
      refreshUnread();
      pollTimer = setInterval(refreshUnread, 30_000);
    }
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    disconnectPersonalWS();
  });

  function logout() {
    disconnectPersonalWS();
    clearSession();
    localStorage.removeItem('resonance:session');
    localStorage.removeItem('resonance:pubkey');
    location.href = '/';
  }

  const navItems = [
    { href: '/feed',         label: 'الخلاصة',    icon: '🌊' },
    { href: '/search',       label: 'بحث',        icon: '🔍' },
    { href: '/discover',     label: 'استكشف',     icon: '✨' },
    { href: '/dms',          label: 'رسائل',      icon: '💬', badge: 'dm' },
    { href: '/notifications',label: 'إشعارات',    icon: '🔔', badge: 'notif' },
    { href: '/saved',        label: 'محفوظات',    icon: '⭐' },
    { href: '/connections',  label: 'صدى',        icon: '🔗' },
    { href: '/traces',       label: 'آثار',       icon: '🌅' },
    { href: '/jury',         label: 'محكمة',      icon: '⚖️' },
    { href: '/settings',     label: 'إعدادات',    icon: '⚙️' },
  ];
</script>

{#if $session.unlocked}
  <nav class="sticky top-0 z-30 backdrop-blur bg-astral-blue-400/80 border-b border-egyptian-gold-400/20">
    <div class="max-w-5xl mx-auto px-4 py-2 flex items-center justify-between">
      <a href="/feed" class="font-serif text-2xl text-egyptian-gold-400 flex items-center gap-2">
        صدى
        <span class="w-2 h-2 rounded-full {$wsConnected ? 'bg-egyptian-gold-400 aura-pulse' : 'bg-papyrus-500'}"></span>
      </a>
      <div class="flex items-center gap-0.5 flex-wrap justify-end">
        {#each navItems as item}
          <a
            href={item.href}
            class="relative flex items-center gap-1.5 px-2.5 py-1.5 rounded text-sm text-papyrus-200 hover:text-egyptian-gold-300 hover:bg-astral-blue-300/50 transition"
          >
            <span>{item.icon}</span>
            <span class="hidden md:inline">{item.label}</span>
            {#if item.badge === 'notif' && unread > 0}
              <span class="absolute -top-0.5 -right-0.5 bg-egyptian-gold-400 text-astral-blue-700 text-[10px] font-bold rounded-full px-1.5 py-0.5 min-w-[16px] text-center">
                {unread > 99 ? '99+' : unread}
              </span>
            {/if}
          </a>
        {/each}
        <button
          on:click={logout}
          class="ml-2 px-2.5 py-1.5 rounded text-sm text-papyrus-400 hover:text-red-300 transition"
        >
          خروج
        </button>
      </div>
    </div>
  </nav>
{/if}
