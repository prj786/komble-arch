<script>
  /**
   * Sheet (design/system/components/Sheet): a tall panel from the right edge
   * with a 48px header, a body that scrolls on its own and an optional footer.
   * bits-ui Dialog gives Esc, click-outside and focus return to the opener;
   * it does not dim the window behind it, and it is non-modal (the README:
   * people keep working beside it) — focus is not trapped, so the list it
   * belongs to stays reachable.
   *
   * `subtitle` is a caption under the title; the `lead` slot puts an icon
   * tile before it. `keepOpen` is a CSS selector: a click outside the sheet
   * on an element inside it does not close the sheet (the list rows that
   * switch what the sheet shows). `initialFocus` is a selector for where
   * focus goes when it opens.
   */
  import { Dialog } from "bits-ui";
  import Icon from "./Icon.svelte";
  export let open = false;
  export let title = "";
  export let subtitle = "";
  export let keepOpen = "";
  // where focus goes on open (a selector inside the sheet); without it, the
  // first control, which is the header's close button
  export let initialFocus = "";
  export let onClose = () => {};

  let content = null;
  function outside(e) {
    const t = e.target;
    if (keepOpen && t instanceof Element && t.closest(keepOpen)) e.preventDefault();
  }
  function autoFocus(e) {
    const el = initialFocus && content ? content.querySelector(initialFocus) : null;
    if (el) {
      e.preventDefault();
      el.focus();
    }
  }
</script>

<Dialog.Root
  {open}
  onOpenChange={(v) => {
    if (!v) onClose();
  }}
>
  <Dialog.Portal>
    <Dialog.Content
      bind:ref={content}
      class="ewe-sheet is-floating"
      trapFocus={false}
      preventScroll={false}
      onInteractOutside={outside}
      onOpenAutoFocus={autoFocus}
    >
      <div class="ewe-sheet__head">
        <slot name="lead" />
        <div class="ewe-sheet__titles">
          <Dialog.Title class="ewe-sheet__title">{title}</Dialog.Title>
          {#if subtitle}<Dialog.Description class="ewe-sheet__subtitle">{subtitle}</Dialog.Description>{/if}
        </div>
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
