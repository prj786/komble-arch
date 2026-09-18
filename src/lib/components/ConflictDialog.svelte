<script>
  // pacman's "X and Y are in conflict. Remove Y?" — the question it would have
  // asked in a terminal, asked here. Store-driven like AppDetail: the Updates
  // view puts {conflicts, resolve} in conflictPrompt and awaits the answer.
  // Dialog card, destructive: a danger icon and button that name what goes,
  // focus on Cancel, Esc cancels, a click on the scrim does nothing.
  import { AlertDialog } from "bits-ui";
  import { conflictPrompt } from "../stores";
  import Icon from "./ui/Icon.svelte";
  const answer = (yes) => {
    const p = $conflictPrompt;
    conflictPrompt.set(null);
    p && p.resolve(yes);
  };
  $: n = $conflictPrompt ? $conflictPrompt.conflicts.length : 0;
  let cancelBtn;
</script>

{#if $conflictPrompt}
  <AlertDialog.Root open={true} onOpenChange={(v) => !v && answer(false)}>
    <AlertDialog.Portal>
      <AlertDialog.Overlay class="scrim" />
      <AlertDialog.Content
        class="ewe-dialog is-floating"
        interactOutsideBehavior="ignore"
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          cancelBtn?.focus();
        }}
      >
        <div class="ewe-dialog__head">
          <span class="ewe-dialog__icon ewe-dialog__icon--danger"><Icon name="warning" /></span>
          <div class="ewe-dialog__titles">
            <AlertDialog.Title class="ewe-dialog__title">Remove {n === 1 ? "a package" : `${n} packages`} to finish the update?</AlertDialog.Title>
            <AlertDialog.Description class="ewe-dialog__desc">
              A newer package replaces or conflicts with something installed. pacman would ask in a terminal; nothing has changed yet.
            </AlertDialog.Description>
          </div>
        </div>
        <div class="ewe-dialog__body">
          {#each $conflictPrompt.conflicts as c (c.remove)}
            <div class="ewe-dialog__who">
              <span class="ewe-mono">{c.remove}</span>
              <span>is replaced by</span>
              <span class="ewe-mono">{c.keep}</span>
              {#if c.reason}<span class="note">{c.reason}</span>{/if}
            </div>
          {/each}
        </div>
        <div class="ewe-dialog__foot">
          <button bind:this={cancelBtn} class="ewe-btn ewe-btn--ghost" on:click={() => answer(false)}>Cancel the update</button>
          <button class="ewe-btn ewe-btn--danger" on:click={() => answer(true)}>
            Remove {n === 1 ? $conflictPrompt.conflicts[0].remove : `${n} packages`} and update
          </button>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>
{/if}
