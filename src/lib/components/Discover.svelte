<script>
  import { catalog, catalogLoading, catalogError } from "../stores";
  import { searchCatalog } from "../fuzzy";
  import { loadCatalog } from "../actions";
  import * as api from "../api";
  import VirtualGrid from "./VirtualGrid.svelte";
  import AppCard from "./AppCard.svelte";

  let query = "";
  let category = "";
  let source = "all"; // all | appimage | deb
  let pkgRepo = "";
  let sections = []; // [name, count]
  let sectionsLoaded = false;
  let pkgResults = [];
  let pkgTotal = 0;
  let pkgSearching = false;
  let indexReady = false;
  let timer;

  const sources = [
    ["all", "All"],
    ["appimage", "AppImages"],
    ["pkg", "Repositories"],
    ["aur", "AUR"]
  ];

  $: cats = [...new Set($catalog.flatMap((i) => i.categories || []))].sort();
  $: aiResults = source === "pkg" ? [] : searchCatalog($catalog, query, category);
  $: if (source === "pkg" && !sectionsLoaded) loadSections();
  $: scheduleDebQuery(query, source, category, pkgRepo);
  $: merged =
    source === "appimage" || (source === "all" && category)
      ? aiResults
      : source === "pkg"
        ? pkgResults
        : [...aiResults, ...pkgResults];

  async function loadSections() {
    sectionsLoaded = true;
    try {
      sections = await api.packageRepos();
      indexReady = true;
    } catch {
      sectionsLoaded = false;
    }
  }

  function toItem(p) {
    return {
      kind: "pkg",
      id: `pkg:${p.name}`,
      pkg: p.name,
      name: p.name,
      description: p.summary,
      plainDesc: p.summary,
      section: p.section,
      version: p.version,
      categories: [],
      installed: p.installed
    };
  }

  function scheduleDebQuery(q, src, cat, sec) {
    clearTimeout(timer);
    const trimmed = q.trim();
    const browsing = src === "pkg";
    if (src === "appimage" || (!browsing && (cat || trimmed.length < 2))) {
      pkgResults = [];
      pkgTotal = 0;
      pkgSearching = false;
      return;
    }
    pkgSearching = true;
    timer = setTimeout(
      async () => {
        try {
          const res = await api.browsePackages(
            trimmed,
            browsing ? sec : "",
            browsing ? 2000 : 400
          );
          indexReady = true;
          if (trimmed === query.trim()) {
            pkgTotal = res.total;
            pkgResults = res.items.map(toItem);
          }
        } catch {
          pkgResults = [];
          pkgTotal = 0;
        }
        pkgSearching = false;
      },
      browsing && !trimmed ? 0 : 250
    );
  }
</script>

<div class="flex h-full flex-col px-6 pt-6">
  <div class="mb-4 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-bold tracking-tight">Discover</h1>
      <p class="text-sm text-zinc-400 dark:text-zinc-500">
        {#if source === "pkg"}
          {#if pkgSearching && !indexReady}
            Building package index…
          {:else if pkgSearching}
            Searching…
          {:else}
            {pkgTotal.toLocaleString()} packages{pkgResults.length < pkgTotal
              ? ` · showing first ${pkgResults.length.toLocaleString()}`
              : ""}
          {/if}
        {:else if $catalogLoading}
          Loading catalog…
        {:else if query.trim()}
          {merged.length.toLocaleString()} result{merged.length === 1 ? "" : "s"}{pkgSearching ? " · searching apt…" : ""}
        {:else}
          {$catalog.length.toLocaleString()} AppImages from AppImageHub · plus your apt repositories
        {/if}
      </p>
    </div>
    <button
      class="btn-ghost"
      title="Refresh catalog"
      disabled={$catalogLoading}
      on:click={() => loadCatalog(true)}
    >
      <svg viewBox="0 0 24 24" class="h-4 w-4 {$catalogLoading ? 'animate-spin' : ''}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
      </svg>
      Refresh
    </button>
  </div>

  <div class="mb-3 flex gap-2.5">
    <input
      class="input flex-1"
      type="search"
      placeholder={source === "pkg"
        ? "Search Debian packages…"
        : "Search AppImages and Debian packages…"}
      bind:value={query}
    />
    {#if source === "pkg"}
      <select class="input w-56" bind:value={pkgRepo}>
        <option value="">All sections</option>
        {#each sections as [s, n]}
          <option value={s}>{s} ({n})</option>
        {/each}
      </select>
    {:else}
      <select class="input w-56" bind:value={category}>
        <option value="">All categories</option>
        {#each cats as c}
          <option value={c}>{c}</option>
        {/each}
      </select>
    {/if}
  </div>

  <div class="mb-4 flex gap-1.5">
    {#each sources as [id, label]}
      <button
        class="rounded-full px-3 py-1 text-xs font-medium transition-colors
          {source === id
          ? 'text-white'
          : 'bg-zinc-200/70 text-zinc-500 hover:bg-zinc-300/70 dark:bg-zinc-700/60 dark:text-zinc-300 dark:hover:bg-zinc-600/60'}"
        style={source === id ? "background: var(--accent)" : ""}
        on:click={() => (source = id)}
      >
        {label}
      </button>
    {/each}
  </div>

  {#if source !== "pkg" && $catalogLoading}
    <div class="grid flex-1 grid-cols-[repeat(auto-fill,minmax(260px,1fr))] content-start gap-3.5 overflow-hidden">
      {#each Array(9) as _}
        <div class="card h-[168px] animate-pulse p-4">
          <div class="flex gap-3">
            <div class="h-11 w-11 rounded-lg bg-zinc-200 dark:bg-zinc-700"></div>
            <div class="flex-1 space-y-2 pt-1">
              <div class="h-3.5 w-2/3 rounded bg-zinc-200 dark:bg-zinc-700"></div>
              <div class="h-2.5 w-1/3 rounded bg-zinc-200 dark:bg-zinc-700"></div>
            </div>
          </div>
          <div class="mt-4 space-y-2">
            <div class="h-2.5 w-full rounded bg-zinc-200 dark:bg-zinc-700"></div>
            <div class="h-2.5 w-4/5 rounded bg-zinc-200 dark:bg-zinc-700"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if source !== "pkg" && $catalogError}
    <div class="flex flex-1 flex-col items-center justify-center gap-3 pb-16 text-center">
      <div class="text-3xl">⚠️</div>
      <p class="max-w-md text-sm text-zinc-500">{$catalogError}</p>
      <button class="btn-primary" on:click={() => loadCatalog(true)}>Try again</button>
    </div>
  {:else if merged.length === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 pb-16 text-center">
      {#if pkgSearching}
        <div class="text-3xl">📦</div>
        <p class="text-sm text-zinc-500">
          {indexReady ? "Searching…" : "Building the package index (first time only)…"}
        </p>
      {:else}
        <div class="text-3xl">🔍</div>
        <p class="text-sm text-zinc-500">Nothing matches “{query}”.</p>
      {/if}
    </div>
  {:else}
    <div class="min-h-0 flex-1">
      <VirtualGrid items={merged} let:item>
        <AppCard {item} />
      </VirtualGrid>
    </div>
  {/if}
</div>
