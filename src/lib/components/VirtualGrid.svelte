<script>
  // Windowed responsive grid: only rows near the viewport are rendered, so
  // the ~1600-item catalog scrolls smoothly. Its sizes come from the tokens
  // (app.css .vgrid: the App card's width is the column minimum, 12px gaps),
  // read off a probe so a look preset or a larger text size moves them; the
  // row height is the first rendered card's own height.
  import { onMount, onDestroy } from "svelte";
  export let items = [];

  let viewport;
  let probe;
  let scrollTop = 0;
  let width = 0;
  let height = 0;
  let minCol = 0;
  let gap = 0;
  let itemHeight = 0;
  let first; // the first rendered cell, measured for the row height

  function measure() {
    if (!probe) return;
    const r = probe.getBoundingClientRect();
    minCol = r.width;
    gap = r.height;
  }
  let ro;
  onMount(() => {
    measure();
    ro = new ResizeObserver(() => {
      measure();
      if (first) itemHeight = first.firstElementChild?.offsetHeight || itemHeight;
    });
    ro.observe(viewport);
    ro.observe(probe);
  });
  onDestroy(() => ro && ro.disconnect());
  $: if (first && ro) {
    ro.observe(first.firstElementChild || first);
    itemHeight = first.firstElementChild?.offsetHeight || itemHeight;
  }

  $: cols = minCol ? Math.max(1, Math.floor((width + gap) / (minCol + gap))) : 1;
  // until the first card is measured, one screenful at a guessed height
  $: rowH = (itemHeight || minCol) + gap;
  $: totalRows = Math.ceil(items.length / cols);
  $: startRow = rowH ? Math.max(0, Math.floor(scrollTop / rowH) - 2) : 0;
  $: endRow = rowH ? Math.min(totalRows, Math.ceil((scrollTop + height) / rowH) + 2) : 1;
  $: visible = items
    .slice(startRow * cols, endRow * cols)
    .map((item, i) => ({ item, idx: startRow * cols + i }));

  // Reset scroll when the item set changes (e.g. new search).
  $: if (items && viewport) {
    viewport.scrollTop = 0;
    scrollTop = 0;
  }
</script>

<div
  bind:this={viewport}
  bind:clientWidth={width}
  bind:clientHeight={height}
  on:scroll={() => (scrollTop = viewport.scrollTop)}
  class="vgrid"
  role="list"
>
  <span class="vgrid__probe" bind:this={probe} aria-hidden="true"></span>
  <div style="height: {totalRows * rowH}px; position: relative;">
    {#each visible as v (v.item.id)}
      {#if v.idx === 0}
        <div
          bind:this={first}
          class="vgrid__cell"
          role="listitem"
          style="top: {Math.floor(v.idx / cols) * rowH}px; left: calc({v.idx % cols} * ((100% - {(cols - 1) * gap}px) / {cols} + {gap}px)); width: calc((100% - {(cols - 1) * gap}px) / {cols});"
        >
          <slot item={v.item} />
        </div>
      {:else}
        <div
          class="vgrid__cell"
          role="listitem"
          style="top: {Math.floor(v.idx / cols) * rowH}px; left: calc({v.idx % cols} * ((100% - {(cols - 1) * gap}px) / {cols} + {gap}px)); width: calc((100% - {(cols - 1) * gap}px) / {cols}); height: {itemHeight ? itemHeight + 'px' : 'auto'};"
        >
          <slot item={v.item} />
        </div>
      {/if}
    {/each}
  </div>
</div>
