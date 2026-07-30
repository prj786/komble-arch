<script>
  import { route, updatesCount } from "../stores";

  const items = [
    {
      id: "discover",
      label: "Discover",
      icon: "M21 21l-4.35-4.35M17 11a6 6 0 1 1-12 0 6 6 0 0 1 12 0z"
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

<aside
  class="flex w-56 shrink-0 flex-col border-r border-zinc-200 bg-white/60 dark:border-zinc-700/60 dark:bg-zinc-800/40"
>
  <div class="flex items-center gap-2.5 px-4 pb-4 pt-5">
    <div
      class="flex h-8 w-8 items-center justify-center rounded-lg text-white"
      style="background: var(--accent)"
    >
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M7 10l5 5 5-5M12 15V3M5 21h14" />
      </svg>
    </div>
    <div class="text-lg font-semibold tracking-tight">Komble</div>
  </div>

  <nav class="flex flex-col gap-0.5 px-2.5">
    {#each items as it}
      <button
        class="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm font-medium transition-colors
          {$route === it.id
          ? 'text-white'
          : 'text-zinc-600 hover:bg-zinc-200/60 dark:text-zinc-300 dark:hover:bg-zinc-700/50'}"
        style={$route === it.id ? "background: var(--accent)" : ""}
        on:click={() => route.set(it.id)}
      >
        <svg viewBox="0 0 24 24" class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d={it.icon} />
        </svg>
        <span class="flex-1 text-left">{it.label}</span>
        {#if it.id === "updates" && $updatesCount > 0}
          <span
            class="rounded-full px-1.5 py-0.5 text-[11px] font-semibold leading-none
              {$route === it.id ? 'bg-white/25 text-white' : 'text-white'}"
            style={$route === it.id ? "" : "background: var(--accent)"}
          >
            {$updatesCount}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="mt-auto px-4 py-3 text-[11px] text-zinc-400 dark:text-zinc-500">
    Komble 0.1.0 · no snap, no flatpak
  </div>
</aside>
