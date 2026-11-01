<script>
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { route, updatesCount } from "../stores";
  import sheep from "../../assets/ewe-mark.svg?raw";
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

  // The side navigation (Side navigation card), grouped by what you came
  // for: getting apps, the apps you have, and the desktop itself (plugins
  // extend the shell, they are not apps). Settings sits at the foot. Lucide
  // glyphs from the same font the shell uses; sentence case.
  const groups = [
    {
      id: "get",
      label: "Get apps",
      items: [
        { id: "discover", label: "Discover", icon: "compass" },
        { id: "foryou", label: "For you", icon: "user" },
        { id: "aur", label: "AUR", icon: "download" }
      ]
    },
    {
      id: "yours",
      label: "Your apps",
      items: [
        { id: "installed", label: "Installed", icon: "package" },
        { id: "updates", label: "Updates", icon: "refresh" }
      ]
    },
    {
      id: "desktop",
      label: "Desktop",
      items: [{ id: "plugins", label: "Plugins", icon: "puzzle" }]
    }
  ];
  const foot = { id: "settings", label: "Settings", icon: "sliders" };
  // the order on screen: Up/Down walk it, Ctrl+1 … Ctrl+7 jump to it
  const items = [...groups.flatMap((g) => g.items), foot];

  let navItems = {};
  function navKey(e, id) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const i = items.findIndex((it) => it.id === id);
    const n = items[(i + d + items.length) % items.length];
    navItems[n.id]?.focus();
    route.set(n.id);
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

{#snippet navItem(it)}
  {@const n = it.id === "updates" ? $updatesCount : 0}
  <button
    bind:this={navItems[it.id]}
    class="ewe-navitem"
    class:is-selected={$route === it.id}
    aria-current={$route === it.id ? "page" : undefined}
    aria-label={n ? `${it.label}, ${n} available` : undefined}
    title={it.label}
    on:click={() => route.set(it.id)}
    on:keydown={(e) => navKey(e, it.id)}
  >
    <Icon name={it.icon} />
    <span class="ewe-navitem__label">{it.label}</span>
    {#if n}
      <!-- a count for updates is the solid accent Badge (Badge card) -->
      <span class="ewe-badge ewe-badge--accent ewe-badge--solid"><span class="ewe-badge__label">{count}</span></span>
      <span class="ewe-badge ewe-badge--accent ewe-badge--solid ewe-badge--dot ewe-badge--ring ewe-navitem__dot" aria-hidden="true"></span>
    {/if}
  </button>
{/snippet}

<!-- the rail: the navigation landmark (App shell, Side navigation); below
     720px of window width it collapses to icons and the headings go -->
<nav class="ewe-sidenav" aria-label="Komble sections">
  <div class="ewe-sidenav__brand">
    <span class="ewe-sidenav__logo" aria-hidden="true">{@html sheep}</span>
    <span class="ewe-sidenav__name">Komble</span>
  </div>

  {#each groups as g (g.id)}
    <div class="ewe-sidenav__group" role="group" aria-labelledby="nav-{g.id}">
      <div class="ewe-sidenav__heading" id="nav-{g.id}">{g.label}</div>
      {#each g.items as it (it.id)}
        {@render navItem(it)}
      {/each}
    </div>
  {/each}

  <div class="ewe-sidenav__foot">
    {@render navItem(foot)}
    <div class="ewe-sidenav__version">Komble{version ? ` ${version}` : ""}</div>
  </div>
</nav>
