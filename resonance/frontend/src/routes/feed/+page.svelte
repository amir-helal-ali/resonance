<script lang="ts">
  import LiveFeed from '$components/LiveFeed.svelte';
  import Composer from '$components/Composer.svelte';
  import TrendingSidebar from '$components/TrendingSidebar.svelte';
  import { session } from '$stores/session';
</script>

<main class="min-h-screen">
  <h1 class="text-center font-serif text-3xl text-egyptian-gold-400 pt-8 pb-2">
    الخلاصة الحيّة
  </h1>
  <p class="text-center text-papyrus-300 text-sm mb-4">
    أول 48 ساعة من كل نبضة — بعد كده بتنتقل للملف الشخصي
  </p>

  <div class="max-w-6xl mx-auto px-4 grid grid-cols-1 lg:grid-cols-3 gap-4">
    <div class="lg:col-span-2">
      {#if $session.unlocked}
        <Composer />
      {/if}
      <LiveFeed />
    </div>
    <div class="hidden lg:block">
      <TrendingSidebar />
    </div>
  </div>
</main>
