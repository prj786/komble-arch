<script>
  // pacman's "X and Y are in conflict. Remove Y?" — the question it would have
  // asked in a terminal, asked here. Store-driven like AppDetail: the Updates
  // view puts {conflicts, resolve} in conflictPrompt and awaits the answer.
  import { conflictPrompt } from "../stores";
  const answer = (yes) => {
    const p = $conflictPrompt;
    conflictPrompt.set(null);
    p && p.resolve(yes);
  };
</script>

<svelte:window on:keydown={(e) => $conflictPrompt && e.key === "Escape" && answer(false)} />

{#if $conflictPrompt}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-3 backdrop-blur-sm sm:p-6" role="dialog" aria-modal="true">
    <div class="card flex w-full max-w-lg flex-col overflow-hidden !bg-white shadow-2xl dark:!bg-[var(--bg-3)]">
      <div class="border-b border-hairline p-5">
        <h2 class="text-base font-semibold">This update removes {$conflictPrompt.conflicts.length === 1 ? "a package" : `${$conflictPrompt.conflicts.length} packages`}</h2>
        <p class="mt-1 text-sm text-dim">
          A newer package replaces or conflicts with something installed. pacman would ask you in a terminal; nothing has been changed yet.
        </p>
      </div>
      <div class="flex flex-col gap-2 p-5">
        {#each $conflictPrompt.conflicts as c (c.remove)}
          <div class="flex items-center gap-3 rounded-xl bg-elevated px-3 py-2 text-sm">
            <span class="font-mono font-semibold">{c.remove}</span>
            <span class="text-dim">→ removed, replaced by</span>
            <span class="font-mono">{c.keep}</span>
            {#if c.reason}<span class="ml-auto text-xs text-dim">{c.reason}</span>{/if}
          </div>
        {/each}
      </div>
      <div class="flex justify-end gap-2 border-t border-hairline p-4">
        <button class="btn-ghost" on:click={() => answer(false)}>Keep them, cancel update</button>
        <button class="btn-primary" on:click={() => answer(true)}>Remove and update</button>
      </div>
    </div>
  </div>
{/if}
