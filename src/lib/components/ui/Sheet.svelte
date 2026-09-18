<script>
  /**
   * Sheet (design/system/components/Sheet): a tall panel from the right edge
   * with a 48px header, a body that scrolls on its own and an optional footer
   * of buttons. bits-ui Dialog gives the focus trap, Esc, click-outside and
   * focus return to the opener; it does not dim the window behind it.
   */
  import { Dialog } from "bits-ui";
  import Icon from "./Icon.svelte";
  export let open = false;
  export let title = "";
  export let onClose = () => {};
</script>

<Dialog.Root
  {open}
  onOpenChange={(v) => {
    if (!v) onClose();
  }}
>
  <Dialog.Portal>
    <Dialog.Content class="ewe-sheet is-floating">
      <div class="ewe-sheet__head">
        <Dialog.Title class="ewe-sheet__title">{title}</Dialog.Title>
        <Dialog.Close class="ewe-iconbtn ewe-iconbtn--ghost" aria-label="Close">
          <Icon name="x" />
        </Dialog.Close>
      </div>
      <div class="ewe-sheet__body"><slot /></div>
      {#if $$slots.footer}
        <div class="ewe-sheet__foot"><slot name="footer" /></div>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
