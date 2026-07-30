<script>
  // Windowed responsive grid: only rows near the viewport are rendered,
  // so the ~1600-item catalog scrolls smoothly.
  export let items = [];
  export let itemHeight = 168;
  export let gap = 14;
  export let minCol = 260;

  let viewport;
  let scrollTop = 0;
  let width = 0;
  let height = 0;

  $: cols = Math.max(1, Math.floor((width + gap) / (minCol + gap)));
  $: rowH = itemHeight + gap;
  $: totalRows = Math.ceil(items.length / cols);
  $: startRow = Math.max(0, Math.floor(scrollTop / rowH) - 2);
  $: endRow = Math.min(totalRows, Math.ceil((scrollTop + height) / rowH) + 2);
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
  class="h-full overflow-y-auto pb-6 pr-1"
>
  <div style="height:{totalRows * rowH}px; position:relative;">
    {#each visible as v (v.item.id)}
      <div
        style="position:absolute;
          top:{Math.floor(v.idx / cols) * rowH}px;
          left:calc({v.idx % cols} * ((100% - {(cols - 1) * gap}px) / {cols} + {gap}px));
          width:calc((100% - {(cols - 1) * gap}px) / {cols});
          height:{itemHeight}px;"
      >
        <slot item={v.item} />
      </div>
    {/each}
  </div>
</div>
