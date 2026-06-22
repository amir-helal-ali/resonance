<script lang="ts">
  import { onMount } from 'svelte';
  import LiveFeed from '$components/LiveFeed.svelte';
  import RegisterForm from '$components/RegisterForm.svelte';
  import { session } from '$stores/session';

  onMount(() => {
    // Restore session from localStorage if present.
    const stored = localStorage.getItem('resonance:session');
    if (stored) {
      try {
        const parsed = JSON.parse(stored);
        session.set({ ...parsed, unlocked: true });
      } catch {
        // ignore
      }
    }
  });

  // Persist session changes.
  $effect(() => {
    if (session.unlocked && session.userId) {
      localStorage.setItem(
        'resonance:session',
        JSON.stringify({ userId: session.userId, username: session.username }),
      );
    }
  });
</script>

<main class="min-h-screen">
  {#if $session.unlocked}
    <LiveFeed />
  {:else}
    <RegisterForm />
  {/if}
</main>
