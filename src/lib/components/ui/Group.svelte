<script>
  /**
   * A group on a settings page: an overline Section header right above a
   * list well of rows. `action` is the header's optional button (bottom
   * right). `well = false` leaves the content bare (cards, grids).
   */
  export let title = "";
  export let desc = "";
  export let well = true;
  let className = "";
  export { className as class };
</script>

<section class="group {className}">
  {#if title || desc || $$slots.action}
    <div class="ewe-section">
      <div class="ewe-section__text">
        {#if title}<h2 class="ewe-section__title ewe-section__title--overline">{title}</h2>{/if}
        {#if desc}<p class="ewe-section__desc">{desc}</p>{/if}
      </div>
      <slot name="action" />
    </div>
  {/if}
  {#if well}
    <div class="ewe-list"><slot /></div>
  {:else}
    <slot />
  {/if}
  <slot name="after" />
</section>
