<script>
  /**
   * Inline alert (design/system/components/InlineAlert): something on this
   * page needs attention or explanation. tone: info | success | warning |
   * danger | accent | "" (neutral). `banner` spans the pane edge to edge.
   */
  import Icon from "./Icon.svelte";
  export let tone = "info";
  export let title = "";
  export let banner = false;
  export let dismiss = null;
  const glyph = { info: "info", accent: "info", success: "success", warning: "warning", danger: "alert" };
</script>

<div
  class="ewe-alert {tone ? `ewe-alert--${tone}` : ''} {banner ? 'ewe-alert--banner' : ''}"
  role={tone === "danger" || tone === "warning" ? "alert" : "status"}
>
  <Icon name={glyph[tone] || "info"} />
  <div class="ewe-alert__body">
    {#if title}<div class="ewe-alert__title">{title}</div>{/if}
    {#if $$slots.default}<div class="ewe-alert__desc"><slot /></div>{/if}
    {#if $$slots.actions}<div class="ewe-alert__actions"><slot name="actions" /></div>{/if}
  </div>
  {#if dismiss}
    <button class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Dismiss" title="Dismiss" on:click={dismiss}>
      <Icon name="x" />
    </button>
  {/if}
</div>
