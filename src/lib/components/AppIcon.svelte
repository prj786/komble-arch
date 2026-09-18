<script>
  // The app icon: the app's own artwork, else the package glyph for a
  // repository package, else its initial on the neutral tile (App card: real
  // icons are the apps' own artwork; the placeholder is .ewe-appicon).
  import Icon from "./ui/Icon.svelte";
  export let item = null;
  export let name = "";
  export let src = "";
  export let pkg = false;
  export let size = ""; // "" (48, the App card) | sm (32, list rows) | xl (64)
  let failed = false;
  $: label = item ? item.name : name;
  $: url = item ? item.icon : src;
  $: isPkg = item ? item.kind === "pkg" : pkg;
  $: url, (failed = false);
</script>

<span class="ewe-appicon {size ? `appicon--${size}` : 'appicon--lg'} {url && !failed ? 'ewe-appicon--img' : ''}" aria-hidden="true">
  {#if url && !failed}
    <img src={url} alt="" loading="lazy" on:error={() => (failed = true)} />
  {:else if isPkg}
    <Icon name="package" />
  {:else}
    {(label || "?").slice(0, 1).toUpperCase()}
  {/if}
</span>
