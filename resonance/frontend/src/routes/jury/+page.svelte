<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/jury/+page.svelte
  // The Transient Jury summons page.
  //
  // When the user is selected as a juror, they see a list of pending panels.
  // Each panel shows the pulse (decrypted client-side after fetching the
  // ciphertext + wrapped key) and two buttons: "أيد" (uphold) or "برّئ" (release).
  // ============================================================================

  import { onMount } from 'svelte';
  import { session } from '$stores/session';
  import { listSummoned, castVote, type JuryOut } from '$lib/api/jury';

  let panels = $state<JuryOut[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function refresh() {
    if (!$session.userId) return;
    loading = true;
    try {
      panels = await listSummoned($session.userId);
    } catch (e: any) {
      error = e?.message ?? 'فشل تحميل الاستدعاءات';
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function handleVote(panel: JuryOut, vote: 'uphold' | 'release') {
    if (!$session.userId) return;
    try {
      const res = await castVote($session.userId, panel.id, vote);
      if (res.final_verdict !== 'pending') {
        // Panel concluded — remove from list.
        panels = panels.filter((p) => p.id !== panel.id);
      } else {
        // Update the panel locally to reflect the vote count.
        await refresh();
      }
    } catch (e: any) {
      error = e?.message ?? 'فشل التصويت';
    }
  }

  function timeLeft(expiresAt: string): string {
    const diff = new Date(expiresAt).getTime() - Date.now();
    const hours = Math.floor(diff / 3_600_000);
    if (hours <= 0) return 'انتهى';
    return `فضل ${hours} ساعة`;
  }
</script>

<main class="max-w-3xl mx-auto py-8 px-4">
  <h1 class="font-serif text-3xl text-egyptian-gold-400 mb-2">المحكمة العابرة</h1>
  <p class="text-papyrus-300 text-sm mb-6">
    اتمسكت كعضو محلف. صوتك بيحمي صدى من السمية ويحفظ حق التعبير.
  </p>

  {#if error}
    <div class="mb-4 p-3 rounded bg-red-900/40 border border-red-500 text-red-200 text-sm">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-center py-12 text-papyrus-300">بيتحمّل...</div>
  {:else if panels.length === 0}
    <div class="text-center py-12">
      <div class="text-5xl mb-3">⚖️</div>
      <p class="text-papyrus-200">مفيش عليك استدعاءات دلوقتي</p>
      <p class="text-papyrus-400 text-sm mt-1">تبقى محلف لما الـ AI يطلب رأي المجتمع</p>
    </div>
  {:else}
    <div class="space-y-4">
      {#each panels as p}
        <article class="p-5 rounded-2xl bg-astral-blue-300/60 border border-egyptian-gold-400/20">
          <header class="flex items-center justify-between mb-3">
            <div>
              <div class="text-xs text-papyrus-400">نبضة رقم</div>
              <div class="font-mono text-sm text-papyrus-100">{p.pulse_id.slice(0, 8)}...</div>
            </div>
            <div class="text-xs text-egyptian-gold-300">{timeLeft(p.expires_at)}</div>
          </header>

          <!-- The pulse content would be decrypted here in production. -->
          <div class="font-serif text-papyrus-100 p-3 rounded bg-astral-blue-500/40 mb-4 text-center text-sm">
            🔒 محتوى مشفّر — مفكوك التشفير يحتاج مفتاح صاحب النبضة
          </div>

          <div class="flex gap-3">
            <button
              on:click={() => handleVote(p, 'release')}
              class="flex-1 py-2.5 rounded bg-egyptian-gold-400/20 text-egyptian-gold-300 font-semibold hover:bg-egyptian-gold-400 hover:text-astral-blue-700 transition"
            >
              برّئ
            </button>
            <button
              on:click={() => handleVote(p, 'uphold')}
              class="flex-1 py-2.5 rounded bg-red-900/40 text-red-200 font-semibold hover:bg-red-800/60 transition"
            >
              أيد الحذف
            </button>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</main>
