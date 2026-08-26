<script>
  import { catalog, catalogLoading, catalogError } from "../stores";
  import { searchCatalog } from "../fuzzy";
  import { loadCatalog } from "../actions";
  import * as api from "../api";
  import VirtualGrid from "./VirtualGrid.svelte";
  import AppCard from "./AppCard.svelte";
  import * as Select from "./ui/select/index.js";

  // "" is a real value here (= no filter), but bits-ui reads "" as "nothing
  // selected" — so it travels under a sentinel, same as ui/SelectRow.svelte.
  const ALL = " all";
  const enc = (v) => (v === "" || v == null ? ALL : String(v));
  const dec = (v) => (v === ALL ? "" : v);

  let query = "";
  let category = "";
  let source = "all"; // all | appimage | pkg | aur
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
  $: repoOpts = [
    ["", "All repositories"],
    ...sections.map(([name, n]) => [name, `${name} (${n})`])
  ];
  $: catOpts = [["", "All categories"], ...cats.map((c) => [c, c])];
  $: repoLabel = (repoOpts.find(([v]) => v === pkgRepo) || repoOpts[0])[1];
  $: catLabel = category || "All categories";
  $: aiResults =
    source === "pkg" || source === "aur" ? [] : searchCatalog($catalog, query, category);
  $: if (source === "pkg" && !sectionsLoaded) loadSections();
  $: schedulePkgQuery(query, source, category, pkgRepo);
  $: merged =
    source === "appimage" || (source === "all" && category)
      ? aiResults
      : source === "pkg" || source === "aur"
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

  function schedulePkgQuery(q, src, cat, sec) {
    clearTimeout(timer);
    const trimmed = q.trim();

    // AUR search goes to the AUR RPC — network, so it needs a real query.
    if (src === "aur") {
      if (trimmed.length < 2) {
        pkgResults = [];
        pkgTotal = 0;
        pkgSearching = false;
        return;
      }
      pkgSearching = true;
      timer = setTimeout(async () => {
        try {
          const res = await api.aurSearch(trimmed);
          if (trimmed === query.trim()) {
            pkgTotal = res.length;
            pkgResults = res.map(toItem);
          }
        } catch {
          pkgResults = [];
          pkgTotal = 0;
        }
        pkgSearching = false;
      }, 300);
      return;
    }

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

<div class="flex h-full flex-col px-4 pt-5 sm:px-6 sm:pt-6">
  <div class="mb-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
    <div class="min-w-0">
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
        {:else if source === "aur"}
          {#if pkgSearching}
            Searching the AUR…
          {:else if query.trim().length >= 2}
            {pkgTotal.toLocaleString()} AUR package{pkgTotal === 1 ? "" : "s"}
          {:else}
            User-submitted packages, built from source after review
          {/if}
        {:else if $catalogLoading}
          Loading catalog…
        {:else if query.trim()}
          {merged.length.toLocaleString()} result{merged.length === 1 ? "" : "s"}{pkgSearching ? " · searching the repos…" : ""}
        {:else}
          {$catalog.length.toLocaleString()} AppImages from the AM catalog · plus the Arch repositories and the AUR
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

  <div class="mb-3 flex flex-col gap-2.5 sm:flex-row">
    <input
      class="input flex-1"
      type="search"
      placeholder={source === "pkg"
        ? "Search the Arch repositories…"
        : source === "aur"
          ? "Search the AUR…"
          : "Search AppImages and Arch packages…"}
      bind:value={query}
    />
    {#if source === "pkg"}
      <Select.Root type="single" value={enc(pkgRepo)} onValueChange={(raw) => (pkgRepo = dec(raw))}>
        <Select.Trigger size="sm" class="w-full sm:w-56">
          <span data-slot="select-value" class="truncate">{repoLabel}</span>
        </Select.Trigger>
        <Select.Content class="max-h-72 p-1">
          {#each repoOpts as [value, label] (enc(value))}
            <Select.Item value={enc(value)} {label} />
          {/each}
        </Select.Content>
      </Select.Root>
    {:else if source !== "aur"}
      <Select.Root type="single" value={enc(category)} onValueChange={(raw) => (category = dec(raw))}>
        <Select.Trigger size="sm" class="w-full sm:w-56">
          <span data-slot="select-value" class="truncate">{catLabel}</span>
        </Select.Trigger>
        <Select.Content class="max-h-72 p-1">
          {#each catOpts as [value, label] (enc(value))}
            <Select.Item value={enc(value)} {label} />
          {/each}
        </Select.Content>
      </Select.Root>
    {/if}
  </div>

  <div class="mb-4 flex flex-wrap gap-1.5">
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

  {#if source !== "pkg" && source !== "aur" && $catalogLoading}
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
  {:else if source !== "pkg" && source !== "aur" && $catalogError}
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
          {source === "aur"
            ? "Searching the AUR…"
            : indexReady
              ? "Searching…"
              : "Building the package index (first time only)…"}
        </p>
      {:else if source === "aur" && query.trim().length < 2}
        <div class="text-3xl">📦</div>
        <p class="text-sm text-zinc-500">Type at least two characters to search the AUR.</p>
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
