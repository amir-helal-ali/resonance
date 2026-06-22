<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/lib/components/Navbar.svelte
  // Top navigation. Shown only when the user is signed in.
  // ============================================================================

  import { session, clearSession } from '$stores/session';

  function logout() {
    clearSession();
    localStorage.removeItem('resonance:session');
    localStorage.removeItem('resonance:pubkey');
    location.href = '/';
  }

  const navItems = [
    { href: '/feed',         label: 'الخلاصة',    icon: '🌊' },
    { href: '/connections',  label: 'صدى',        icon: '🔗' },
    { href: '/traces',       label: 'الآثار',     icon: '🌅' },
    { href: '/jury',         label: 'المحكمة',    icon: '⚖️' },
    { href: '/profile',      label: 'ملفي',       icon: '🪶' },
  ];
</script>

{#if $session.unlocked}
  <nav class="sticky top-0 z-30 backdrop-blur bg-astral-blue-400/80 border-b border-egyptian-gold-400/20">
    <div class="max-w-5xl mx-auto px-4 py-3 flex items-center justify-between">
      <a href="/feed" class="font-serif text-2xl text-egyptian-gold-400">صدى</a>
      <div class="flex items-center gap-1">
        {#each navItems as item}
          <a
            href={item.href}
            class="flex items-center gap-1.5 px-3 py-1.5 rounded text-sm text-papyrus-200 hover:text-egyptian-gold-300 hover:bg-astral-blue-300/50 transition"
          >
            <span>{item.icon}</span>
            <span class="hidden sm:inline">{item.label}</span>
          </a>
        {/each}
        <button
          on:click={logout}
          class="ml-2 px-3 py-1.5 rounded text-sm text-papyrus-400 hover:text-red-300 transition"
        >
          خروج
        </button>
      </div>
    </div>
  </nav>
{/if}
