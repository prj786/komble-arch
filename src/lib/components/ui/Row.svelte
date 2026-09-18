<script>
  /**
   * A settings row (Settings page, List row): the setting's name and what it
   * does on the left, its control or current value on the right.
   * `icon` adds the 32px icon tile; `dim` marks the row unavailable (text in
   * text-disabled, the control still says why through its own state);
   * `block` stacks the control under the text (sliders, pickers).
   */
  import Icon from "./Icon.svelte";
  export let title = "";
  export let sub = "";
  export let icon = "";
  export let dim = false;
  export let block = false;
  export let dense = false;
  let className = "";
  export { className as class };
</script>

<div
  class="ewe-row {block ? 'ewe-row--block' : ''} {dense ? 'ewe-row--dense' : ''} {dim ? 'is-dim pointer-events-none' : ''} {className}"
  aria-disabled={dim || undefined}
>
  {#if icon}
    <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={icon} /></span>
  {/if}
  {#if title || sub || $$slots.text}
    <div class="ewe-row__text">
      {#if title}<div class="ewe-row__title">{title}</div>{/if}
      {#if sub}<div class="ewe-row__desc">{sub}</div>{/if}
      <slot name="text" />
    </div>
  {/if}
  {#if $$slots.default}
    <div class="ewe-row__trail"><slot /></div>
  {/if}
</div>
