<script lang="ts">
  import '../app.css';
  let { children } = $props();
</script>

<svelte:head>
  <title>صدى — Resonance</title>
  <meta name="description" content="صدى — منصة التواصل الاجتماعي الزمنية الخاصة" />
</svelte:head>

<!-- The root layout is the deep astral blue night sky. -->
<div class="min-h-screen bg-astral-blue-400 text-papyrus-50">
  {@render children()}
</div>
