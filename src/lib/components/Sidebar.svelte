<script>
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { route, updatesCount } from "../stores";

  // Real app version, not a hardcoded string that goes stale on release.
  let version = "";
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // dev server outside Tauri
    }
  });

  const items = [
    {
      id: "discover",
      label: "Discover",
      icon: "M21 21l-4.35-4.35M17 11a6 6 0 1 1-12 0 6 6 0 0 1 12 0z"
    },
    {
      id: "foryou",
      label: "For you",
      icon: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z"
    },
    {
      id: "installed",
      label: "Installed",
      icon: "M20 7l-8-4-8 4v10l8 4 8-4V7zM4 7l8 4m0 0l8-4m-8 4v10"
    },
    {
      id: "updates",
      label: "Updates",
      icon: "M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
    },
    {
      id: "aur",
      label: "AUR",
      icon: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"
    },
    {
      id: "settings",
      label: "Settings",
      icon: "M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6"
    }
  ];
</script>

<!-- Collapses to an icon rail on narrow windows (tiled half/quarter screens). -->
<aside
  class="flex w-14 shrink-0 flex-col border-r border-zinc-200 bg-white/60 md:w-56 dark:border-zinc-700/60 dark:bg-zinc-800/40"
>
  <div class="flex items-center justify-center gap-2.5 px-2 pb-4 pt-5 md:justify-start md:px-4">
    <!-- the kombali — a shepherd's crook; Komble herds your apps -->
    <div
      class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-white"
      style="background: linear-gradient(135deg, #f0a65c, #744664)"
    >
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M14.5 21V8a3.5 3.5 0 0 0-7 0v1.5" />
      </svg>
    </div>
    <div class="hidden text-lg font-semibold tracking-tight md:block">Komble</div>
  </div>

  <nav class="flex flex-col gap-0.5 px-1.5 md:px-2.5">
    {#each items as it}
      <button
        title={it.label}
        class="relative flex items-center justify-center gap-2.5 rounded-lg px-2.5 py-2 text-sm font-medium transition-colors md:justify-start
          {$route === it.id
          ? 'text-white'
          : 'text-zinc-600 hover:bg-zinc-200/60 dark:text-zinc-300 dark:hover:bg-zinc-700/50'}"
        style={$route === it.id ? "background: var(--accent)" : ""}
        on:click={() => route.set(it.id)}
      >
        <svg viewBox="0 0 24 24" class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d={it.icon} />
        </svg>
        <span class="hidden flex-1 text-left md:block">{it.label}</span>
        {#if it.id === "updates" && $updatesCount > 0}
          <span
            class="hidden rounded-full px-1.5 py-0.5 text-[11px] font-semibold leading-none md:inline
              {$route === it.id ? 'bg-white/25 text-white' : 'text-white'}"
            style={$route === it.id ? "" : "background: var(--accent)"}
          >
            {$updatesCount}
          </span>
          <!-- icon-rail equivalent of the count badge -->
          <span
            class="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full md:hidden
              {$route === it.id ? 'bg-white' : ''}"
            style={$route === it.id ? "" : "background: var(--accent)"}
          ></span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="mt-auto hidden px-4 py-3 text-[11px] text-zinc-400 md:block dark:text-zinc-500">
    Komble{version ? ` ${version}` : ""} · no snap, no flatpak
  </div>
</aside>
