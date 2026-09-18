<script>
  /**
   * Segmented control (design/system/components/SegmentedControl): one choice
   * out of a few, in a surface-sunken well; the selected item is
   * surface-selected. A radio group: Tab reaches the selected item, the
   * arrow keys move and pick.
   * options: [{ value, label }] or [[value, label]].
   */
  export let options = [];
  export let value;
  export let label = "";
  export let disabled = false;
  export let size = "";
  export let block = false;
  export let accent = false; // the one primary filter in a view
  export let picked = () => {};

  $: opts = options.map((o) => (Array.isArray(o) ? { value: o[0], label: o[1] } : o));
  $: current = opts.findIndex((o) => String(o.value) === String(value));
  let items = [];

  function key(e, i) {
    const d = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + opts.length) % opts.length;
    items[n]?.focus();
    picked(opts[n].value);
  }
</script>

<div
  class="ewe-seg {size ? `ewe-seg--${size}` : ''} {block ? 'ewe-seg--block' : ''} {accent ? 'ewe-seg--accent' : ''}"
  role="radiogroup"
  aria-label={label || undefined}
>
  {#each opts as o, i (String(o.value))}
    <button
      bind:this={items[i]}
      type="button"
      role="radio"
      aria-checked={i === current}
      tabindex={i === current || (current < 0 && i === 0) ? 0 : -1}
      class="ewe-seg__item"
      class:is-selected={i === current}
      {disabled}
      on:click={() => i !== current && picked(o.value)}
      on:keydown={(e) => key(e, i)}>{o.label}</button
    >
  {/each}
</div>
