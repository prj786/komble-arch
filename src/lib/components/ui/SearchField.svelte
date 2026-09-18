<script>
  /**
   * Search field (design/system/components/SearchField): the Text field box
   * with the search glyph first and a clear button once there is text. Esc
   * clears the query.
   */
  import Icon from "./Icon.svelte";
  export let value = "";
  export let placeholder = "Search";
  export let autofocus = false;
  let input;
  import { onMount } from "svelte";
  onMount(() => autofocus && input?.focus());
</script>

<label class="ewe-input">
  <Icon name="search" />
  <input
    bind:this={input}
    bind:value
    class="ewe-input__text search-text"
    type="search"
    {placeholder}
    aria-label={placeholder}
    on:keydown={(e) => {
      if (e.key === "Escape" && value) {
        e.stopPropagation();
        value = "";
      }
    }}
  />
  {#if value}
    <button type="button" class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Clear" on:click={() => ((value = ""), input?.focus())}>
      <Icon name="x" />
    </button>
  {/if}
</label>
