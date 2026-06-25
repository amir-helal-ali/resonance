<script lang="ts">
  // ============================================================================
  // resonance-frontend/src/routes/settings/+page.svelte
  // Profile editing, key rotation, block management, account deletion.
  // ============================================================================

  import { onMount } from 'svelte';
  import { session, clearSession } from '$stores/session';
  import {
    updateProfile,
    listBlocks,
    blockUser,
    unblockUser,
    deleteAccount,
    type BlockOut,
  } from '$lib/api/settings';

  let imprint = $state('');
  let horizon = $state('');
  let savedMsg = $state('');
  let blocks = $state<BlockOut[]>([]);
  let blockInput = $state('');
  let confirmDelete = $state(false);
  let loading = $state(true);

  async function load() {
    if (!$session.userId) return;
    loading = true;
    try {
      blocks = await listBlocks($session.userId);
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function handleSaveProfile() {
    if (!$session.userId) return;
    try {
      await updateProfile($session.userId, { imprint, horizon });
      savedMsg = 'اتحفظ ✨';
      setTimeout(() => (savedMsg = ''), 2000);
    } catch (e: any) {
      savedMsg = 'فشل الحفظ';
    }
  }

  async function handleBlock() {
    if (!$session.userId || !blockInput.trim()) return;
    try {
      await blockUser($session.userId, blockInput.trim());
      blockInput = '';
      await load();
    } catch (e: any) {
      console.error(e);
    }
  }

  async function handleUnblock(userId: string) {
    if (!$session.userId) return;
    try {
      await unblockUser($session.userId, userId);
      blocks = blocks.filter((b) => b.user_id !== userId);
    } catch (e: any) {
      console.error(e);
    }
  }

  async function handleDelete() {
    if (!$session.userId) return;
    try {
      await deleteAccount($session.userId);
      clearSession();
      localStorage.clear();
      location.href = '/';
    } catch (e: any) {
      console.error(e);
    }
  }
</script>

<main class="max-w-2xl mx-auto py-8 px-4 space-y-8">
  <h1 class="font-serif text-3xl text-egyptian-gold-400">الإعدادات</h1>

  <!-- Profile -->
  <section class="p-5 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
    <h2 class="font-serif text-xl text-egyptian-gold-300 mb-4">الملف الشخصي</h2>

    <label class="block text-sm text-papyrus-200 mb-1">بصمتي</label>
    <textarea
      bind:value={imprint}
      rows="3"
      maxlength="500"
      placeholder="إيه اللي بيميّزك؟"
      class="w-full bg-astral-blue-500 text-papyrus-50 p-3 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 mb-4 resize-none font-serif"
    ></textarea>

    <label class="block text-sm text-papyrus-200 mb-1">أُفقي</label>
    <textarea
      bind:value={horizon}
      rows="3"
      maxlength="500"
      placeholder="أهدافك؟ أحلامك؟"
      class="w-full bg-astral-blue-500 text-papyrus-50 p-3 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 mb-4 resize-none font-serif"
    ></textarea>

    <div class="flex items-center gap-3">
      <button
        on:click={handleSaveProfile}
        class="px-5 py-2 rounded bg-egyptian-gold-400 text-astral-blue-700 font-semibold hover:bg-egyptian-gold-300 transition"
      >
        حفظ
      </button>
      {#if savedMsg}
        <span class="text-sm text-egyptian-gold-300">{savedMsg}</span>
      {/if}
    </div>
  </section>

  <!-- Blocks -->
  <section class="p-5 rounded-2xl bg-astral-blue-300/40 border border-egyptian-gold-400/20">
    <h2 class="font-serif text-xl text-egyptian-gold-300 mb-4">المحظورين</h2>

    <div class="flex gap-2 mb-4">
      <input
        bind:value={blockInput}
        placeholder="user id توعوز تحظره..."
        class="flex-1 bg-astral-blue-500 text-papyrus-50 px-3 py-2 rounded border border-egyptian-gold-400/20 focus:outline-none focus:border-egyptian-gold-400 text-sm"
      />
      <button
        on:click={handleBlock}
        class="px-4 py-2 rounded bg-red-900/40 text-red-200 text-sm hover:bg-red-800/60 transition"
      >
        احظر
      </button>
    </div>

    {#if blocks.length === 0}
      <p class="text-sm text-papyrus-400">مفيش حد محظور.</p>
    {:else}
      <div class="space-y-2">
        {#each blocks as b}
          <div class="flex items-center justify-between p-2 rounded bg-astral-blue-500/40">
            <span class="text-papyrus-100 text-sm font-mono">{b.username}</span>
            <button
              on:click={() => handleUnblock(b.user_id)}
              class="text-xs text-papyrus-400 hover:text-egyptian-gold-300 transition"
            >
              فك الحظر
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Danger zone -->
  <section class="p-5 rounded-2xl bg-red-900/20 border border-red-500/30">
    <h2 class="font-serif text-xl text-red-300 mb-2">منطقة الخطر</h2>
    <p class="text-sm text-papyrus-300 mb-4">
      حذف الحساب بيمسح كل نبضاتك ومحادثاتك وآثارك نهائياً. مفيش رجوع.
    </p>

    {#if !confirmDelete}
      <button
        on:click={() => (confirmDelete = true)}
        class="px-4 py-2 rounded bg-red-900/60 text-red-200 text-sm hover:bg-red-800/80 transition"
      >
        احذف حسابي
      </button>
    {:else}
      <div class="flex gap-2">
        <button
          on:click={handleDelete}
          class="px-4 py-2 rounded bg-red-700 text-white text-sm hover:bg-red-600 transition"
        >
          أكد الحذف النهائي
        </button>
        <button
          on:click={() => (confirmDelete = false)}
          class="px-4 py-2 rounded bg-astral-blue-500 text-papyrus-200 text-sm hover:bg-astral-blue-400 transition"
        >
          إلغاء
        </button>
      </div>
    {/if}
  </section>
</main>
