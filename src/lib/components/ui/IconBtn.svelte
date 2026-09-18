<script>
  /**
   * Icon button (design/system/components/IconButton), ghost by default, with
   * the required Tooltip naming the action. `name` is a glyph from icons.js;
   * `icon` (inner SVG markup) is still accepted from older call sites.
   * `danger` tints the hover like a danger menu row, for Remove / Forget.
   */
  import { Tooltip } from "bits-ui";
  import Icon from "./Icon.svelte";
  export let name = "";
  export let icon = "";
  export let title = "";
  export let disabled = false;
  export let danger = false;
  export let selected = false;
  export let variant = "ghost"; // ghost | secondary | primary
  export let size = "sm"; // sm | md | lg
  export let go = () => {};
</script>

<Tooltip.Root delayDuration={400}>
  <Tooltip.Trigger {disabled}>
    {#snippet child({ props })}
      <button
        {...props}
        type="button"
        aria-label={title}
        {disabled}
        class="ewe-iconbtn ewe-iconbtn--{variant} {size !== 'md' ? `ewe-iconbtn--${size}` : ''} {danger ? 'is-danger' : ''}"
        class:is-selected={selected}
        aria-pressed={selected || undefined}
        on:click={() => go()}
      >
        {#if name}
          <Icon {name} />
        {:else}
          <svg viewBox="0 0 24 24" class="ewe-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            {@html icon}
          </svg>
        {/if}
      </button>
    {/snippet}
  </Tooltip.Trigger>
  {#if title}
    <Tooltip.Portal>
      <Tooltip.Content sideOffset={4} class="ewe-tooltip">{title}</Tooltip.Content>
    </Tooltip.Portal>
  {/if}
</Tooltip.Root>
