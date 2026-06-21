<script lang="ts">
  import LiveFeed from '$components/LiveFeed.svelte';
</script>

<main class="min-h-screen">
  <h1 class="text-center font-serif text-3xl text-egyptian-gold-400 pt-8 pb-2">
    الخلاصة الحيّة
  </h1>
  <p class="text-center text-papyrus-300 text-sm mb-4">
    أول 48 ساعة من كل نبضة — بعد كده بتنتقل للملف الشخصي
  </p>
  <LiveFeed />
</main>
