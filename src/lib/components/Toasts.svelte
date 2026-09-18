<script>
  /**
   * The Toast (design/system/components/Toast): one at a time, centered at
   * the bottom of the window. The 2px accent timer shrinks while the toast
   * waits and pauses while it is hovered or focused; when it runs out the
   * toast closes. A message that is still going on ("Installing…") shows a
   * small Spinner in place of the icon. role=status: announced politely,
   * never takes focus.
   */
  import { currentToast, dismissToast } from "../stores.js";
  import Icon from "./ui/Icon.svelte";

  const glyph = { success: "success", warning: "warning", danger: "alert", info: "info" };
  // **name** in a message is the thing it names, set in weight 600
  const parts = (m) => String(m).split(/\*\*(.+?)\*\*/g).map((t, i) => ({ t, b: i % 2 === 1 }));
  const ongoing = (t) => t.tone === "info" && /…$/.test(t.message.trim());

  async function act(t) {
    dismissToast(t.id);
    try {
      await t.action.run();
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class="toasts" role="status" aria-live="polite">
  {#if $currentToast}
    {#key $currentToast.id}
      {@const t = $currentToast}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ewe-toast ewe-toast--{t.tone}"
        on:keydown={(e) => e.key === "Escape" && dismissToast(t.id)}
      >
        {#if ongoing(t)}
          <span class="ewe-spinner" aria-hidden="true"></span>
        {:else}
          <Icon name={glyph[t.tone] || "info"} />
        {/if}
        <span class="ewe-toast__text toast-clamp" title={t.message}>
          {#each parts(t.message) as p}{#if p.b}<b>{p.t}</b>{:else}{p.t}{/if}{/each}
        </span>
        {#if t.action}
          <button class="ewe-toast__action" on:click={() => act(t)}>{t.action.label}</button>
        {/if}
        <span class="ewe-toast__sep"></span>
        <button class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Close" on:click={() => dismissToast(t.id)}>
          <Icon name="x" />
        </button>
        <span
          class="ewe-toast__timer"
          style="animation-duration: {t.timeout}ms"
          on:animationend={() => dismissToast(t.id)}
        ></span>
      </div>
    {/key}
  {/if}
</div>
