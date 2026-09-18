<script>
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { route, updatesCount } from "../stores";
  import sheep from "../../assets/sheep.svg?raw";
  import Icon from "./ui/Icon.svelte";

  // Real app version, not a hardcoded string that goes stale on release.
  let version = "";
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // dev server outside Tauri
    }
  });

  // The side navigation (Side navigation card). Lucide glyphs from the same
  // font the shell uses; sentence case.
  const items = [
    { id: "discover", label: "Discover", icon: "compass" },
    { id: "foryou", label: "For you", icon: "user" },
    { id: "installed", label: "Installed", icon: "package" },
    { id: "plugins", label: "Plugins", icon: "puzzle" },
    { id: "updates", label: "Updates", icon: "refresh" },
    { id: "aur", label: "AUR", icon: "download" },
    { id: "settings", label: "Settings", icon: "sliders" }
  ];

  // Up and Down move between sections; Ctrl+1 … Ctrl+7 jump to them.
  let navItems = [];
  function navKey(e, i) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + items.length) % items.length;
    navItems[n]?.focus();
    route.set(items[n].id);
  }
  function globalKey(e) {
    if (e.ctrlKey && !e.altKey && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      const it = items[Number(e.key) - 1];
      if (it) {
        e.preventDefault();
        route.set(it.id);
      }
    }
  }
  // "99+" above 99 (Badge card)
  $: count = $updatesCount > 99 ? "99+" : String($updatesCount);
</script>

<svelte:window on:keydown={globalKey} />

<!-- the rail: the navigation landmark (App shell, Side navigation); below
     720px of window width it collapses to icons -->
<nav class="ewe-sidenav" aria-label="Komble sections">
  <div class="ewe-sidenav__brand">
    <span class="ewe-sidenav__logo" aria-hidden="true">{@html sheep}</span>
    <span class="ewe-sidenav__name">Komble</span>
  </div>

  <div class="ewe-sidenav__group">
    {#each items as it, i (it.id)}
      {@const n = it.id === "updates" ? $updatesCount : 0}
      <button
        bind:this={navItems[i]}
        class="ewe-navitem"
        class:is-selected={$route === it.id}
        aria-current={$route === it.id ? "page" : undefined}
        aria-label={n ? `${it.label}, ${n} available` : undefined}
        title={it.label}
        on:click={() => route.set(it.id)}
        on:keydown={(e) => navKey(e, i)}
      >
        <Icon name={it.icon} />
        <span class="ewe-navitem__label">{it.label}</span>
        {#if n}
          <!-- a count for updates is the solid accent Badge (Badge card) -->
          <span class="ewe-badge ewe-badge--accent ewe-badge--solid"><span class="ewe-badge__label">{count}</span></span>
          <span class="ewe-badge ewe-badge--accent ewe-badge--solid ewe-badge--dot ewe-badge--ring ewe-navitem__dot" aria-hidden="true"></span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="ewe-sidenav__foot">
    <div class="ewe-sidenav__version">Komble{version ? ` ${version}` : ""}</div>
    <p class="ewe-sidenav__tagline">No snap, no flatpak</p>
  </div>
</nav>
