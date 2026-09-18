<script>
  import { catalog, catalogLoading, catalogError, pendingSearch } from "../stores";
  import { searchCatalog } from "../fuzzy";
  import { loadCatalog } from "../actions";
  import * as api from "../api";
  import VirtualGrid from "./VirtualGrid.svelte";
  import AppCard from "./AppCard.svelte";
  import * as Select from "./ui/select/index.js";
  import Page from "./ui/Page.svelte";
  import Seg from "./ui/Seg.svelte";
  import SearchField from "./ui/SearchField.svelte";
  import Icon from "./ui/Icon.svelte";

  // "" is a real value here (= no filter), but bits-ui reads "" as "nothing
  // selected" — so it travels under a sentinel, same as ui/SelectRow.svelte.
  const ALL = " all";
  const enc = (v) => (v === "" || v == null ? ALL : String(v));
  const dec = (v) => (v === ALL ? "" : v);

  let query = "";
  // opened INTO a search (`komble --search=pdf`): fill the box once
  $: if ($pendingSearch) {
    query = $pendingSearch;
    pendingSearch.set("");
  }
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

  $: q2 = query.trim();
  $: subtitle =
    source === "pkg"
      ? pkgSearching && !indexReady
        ? "Building the package index…"
        : pkgSearching
          ? "Searching…"
          : `${pkgTotal.toLocaleString()} packages${pkgResults.length < pkgTotal ? ` · showing the first ${pkgResults.length.toLocaleString()}` : ""}`
      : source === "aur"
        ? pkgSearching
          ? "Searching the AUR…"
          : q2.length >= 2
            ? `${pkgTotal.toLocaleString()} AUR package${pkgTotal === 1 ? "" : "s"}`
            : "User-submitted packages, built from source after you review them"
        : $catalogLoading
          ? "Loading the catalog…"
          : q2
            ? `${merged.length.toLocaleString()} result${merged.length === 1 ? "" : "s"}${pkgSearching ? " · searching the repositories…" : ""}`
            : `${$catalog.length.toLocaleString()} AppImages from the AM catalog, plus the Arch repositories and the AUR`;

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

<Page title="Discover" desc={subtitle} fill>
  <svelte:fragment slot="actions">
    <!-- Button, loading: a Spinner replaces the icon, the label says what runs -->
    <button
      class="ewe-btn ewe-btn--secondary"
      disabled={$catalogLoading}
      aria-busy={$catalogLoading}
      on:click={() => loadCatalog(true)}
    >
      {#if $catalogLoading}
        <span class="ewe-spinner ewe-spinner--sm ewe-spinner--neutral" aria-hidden="true"></span>Refreshing…
      {:else}
        <Icon name="refresh" />Refresh
      {/if}
    </button>
  </svelte:fragment>

  <div class="toolbar">
    <SearchField
      bind:value={query}
      placeholder={source === "pkg"
        ? "Search the Arch repositories"
        : source === "aur"
          ? "Search the AUR"
          : "Search AppImages and Arch packages"}
    />
    {#if source === "pkg"}
      <Select.Root type="single" value={enc(pkgRepo)} onValueChange={(raw) => (pkgRepo = dec(raw))}>
        <Select.Trigger class="select-trigger" aria-label="Repository">{repoLabel}</Select.Trigger>
        <Select.Content>
          {#each repoOpts as [value, label] (enc(value))}
            <Select.Item value={enc(value)} {label} />
          {/each}
        </Select.Content>
      </Select.Root>
    {:else if source !== "aur"}
      <Select.Root type="single" value={enc(category)} onValueChange={(raw) => (category = dec(raw))}>
        <Select.Trigger class="select-trigger" aria-label="Category">{catLabel}</Select.Trigger>
        <Select.Content>
          {#each catOpts as [value, label] (enc(value))}
            <Select.Item value={enc(value)} {label} />
          {/each}
        </Select.Content>
      </Select.Root>
    {/if}
    <!-- the one primary filter of the view: the accent Segmented control -->
    <Seg options={sources} value={source} label="Source" accent picked={(v) => (source = v)} />
  </div>

  {#if source !== "pkg" && source !== "aur" && $catalogLoading}
    <!-- Skeleton: the grid's own shape, nine App cards -->
    <div class="grid-static" aria-busy="true" aria-label="Loading the catalog">
      {#each Array(9) as _}
        <div class="ewe-appcard" aria-hidden="true">
          <div class="ewe-appcard__top">
            <span class="ewe-skel ewe-skel--block appicon--lg"></span>
            <div class="ewe-appcard__titles">
              <span class="ewe-skel ewe-skel--title w-3/5"></span>
              <span class="ewe-skel ewe-skel--text w-2/5"></span>
            </div>
          </div>
          <div class="ewe-appcard__summary">
            <span class="ewe-skel ewe-skel--text"></span>
            <span class="ewe-skel ewe-skel--text w-11/12"></span>
          </div>
          <div class="ewe-appcard__foot"><span class="ewe-skel ewe-skel--text w-3/5"></span></div>
        </div>
      {/each}
    </div>
  {:else if source !== "pkg" && source !== "aur" && $catalogError}
    <div class="ewe-empty" role="alert">
      <span class="ewe-empty__icon"><Icon name="alert" /></span>
      <div class="ewe-empty__title">Couldn’t load the catalog</div>
      <div class="ewe-empty__desc">{$catalogError}</div>
      <div class="ewe-empty__actions">
        <button class="ewe-btn ewe-btn--secondary" on:click={() => loadCatalog(true)}>Try again</button>
      </div>
    </div>
  {:else if merged.length === 0}
    <div class="ewe-empty" aria-live="polite">
      {#if pkgSearching}
        <span class="ewe-empty__icon"><span class="ewe-spinner ewe-spinner--xl" aria-hidden="true"></span></span>
        <div class="ewe-empty__title">
          {source === "aur" ? "Searching the AUR…" : indexReady ? "Searching…" : "Building the package index…"}
        </div>
        {#if !indexReady && source !== "aur"}<div class="ewe-empty__desc">This happens only the first time.</div>{/if}
      {:else if source === "aur" && query.trim().length < 2}
        <span class="ewe-empty__icon"><Icon name="search" /></span>
        <div class="ewe-empty__title">Search the AUR</div>
        <div class="ewe-empty__desc">Type at least two characters.</div>
      {:else}
        <span class="ewe-empty__icon"><Icon name="search" /></span>
        <div class="ewe-empty__title">No apps match “{query}”</div>
        <div class="ewe-empty__desc">Check the spelling, or search the AUR.</div>
      {/if}
    </div>
  {:else}
    <VirtualGrid items={merged} let:item>
      <AppCard {item} />
    </VirtualGrid>
  {/if}
</Page>
