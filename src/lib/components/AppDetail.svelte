<script>
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    selectedApp,
    installedIds,
    progress,
    route,
    settings,
    trackedPkgs,
    aurReview,
    toast
  } from "../stores";
  import { refreshInstalled, refreshPkgs } from "../actions";
  import * as api from "../api";
  import { stripHtml, formatBytes } from "../utils";

  let release = null;
  let pkgInfo = null;
  let relError = "";
  let loading = false;
  let iconError = false;
  let confirming = false;
  let pkgBusy = false;
  let lastId = null;

  $: item = $selectedApp;
  $: isPkg = item?.kind === "pkg";
  $: isAur = isPkg && item.section === "aur";
  $: prog = item && !isPkg && $progress[item.id];
  $: isInstalled = item
    ? isPkg
      ? item.installed || $trackedPkgs.some((d) => d.package === item.pkg)
      : $installedIds.has(item.id)
    : false;
  $: pct = prog && prog.total > 0 ? Math.min(100, Math.round((prog.downloaded / prog.total) * 100)) : null;
  $: if (item && item.id !== lastId) loadDetails(item);

  async function loadDetails(it) {
    lastId = it.id;
    release = null;
    pkgInfo = null;
    relError = "";
    iconError = false;
    confirming = false;
    loading = true;
    try {
      if (it.kind === "pkg") {
        // pacman only knows an AUR package once it is installed (-Qi); before
        // that, the search result itself is all the detail there is.
        if (it.section !== "aur" || it.installed) {
          pkgInfo = await api.packageInfo(it.pkg);
        }
      } else if (it.github) {
        release = await api.resolveRelease(it.github, $settings.githubToken);
      } else if (it.source === "am") {
        const r = await api.resolveAmApp(it.id, $settings.githubToken);
        release = r.release;
        // remember the repo so install records it and updates get tracked
        if (r.github) it.github = r.github;
      }
    } catch (e) {
      relError = String(e);
    }
    loading = false;
  }

  function close() {
    selectedApp.set(null);
    lastId = null;
  }

  async function install() {
    if (prog || isInstalled || pkgBusy) return;
    // AUR: hand over to the review gate — never a one-click build.
    if (isAur) {
      aurReview.set(item.pkg);
      route.set("aur");
      close();
      return;
    }
    if (isPkg) {
      pkgBusy = true;
      try {
        toast(`Installing ${item.pkg} — authentication may be required…`, "info");
        await api.installPackage(item.pkg);
        toast(`${item.pkg} installed`, "success");
        item.installed = true;
        refreshPkgs();
      } catch (e) {
        toast(e, "error");
      }
      pkgBusy = false;
      return;
    }
    try {
      await api.installFromItem(item, $settings);
      toast(`${item.name} installed`, "success");
    } catch (e) {
      toast(e, "error");
    }
  }

  async function remove() {
    if (!confirming) {
      confirming = true;
      setTimeout(() => (confirming = false), 4000);
      return;
    }
    confirming = false;
    try {
      if (isPkg) {
        pkgBusy = true;
        toast(`Removing ${item.pkg} — authentication may be required…`, "info");
        await api.removePackage(item.pkg);
        toast(`${item.pkg} removed`, "success");
        item.installed = false;
        refreshPkgs();
        pkgBusy = false;
      } else {
        await api.removeAppimage(item.id);
        toast(`${item.name} removed`, "success");
        refreshInstalled();
      }
    } catch (e) {
      toast(e, "error");
      pkgBusy = false;
    }
  }
</script>

<svelte:window on:keydown={(e) => e.key === "Escape" && close()} />

{#if item}
  <div
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-3 backdrop-blur-sm sm:p-6"
    role="button"
    tabindex="-1"
    on:click|self={close}
    on:keydown={() => {}}
  >
    <div class="card flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden !bg-white shadow-2xl dark:!bg-zinc-900">
      <div class="flex items-start gap-3 border-b border-zinc-200 p-4 sm:gap-4 sm:p-5 dark:border-zinc-700/60">
        {#if isPkg}
          <div class="flex h-16 w-16 items-center justify-center rounded-xl bg-zinc-200 text-zinc-500 dark:bg-zinc-700 dark:text-zinc-300">
            <svg viewBox="0 0 24 24" class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 7l-8-4-8 4v10l8 4 8-4V7zM4 7l8 4m0 0l8-4m-8 4v10" />
            </svg>
          </div>
        {:else if item.icon && !iconError}
          <img src={item.icon} alt="" class="h-16 w-16 rounded-xl object-contain" on:error={() => (iconError = true)} />
        {:else}
          <div class="flex h-16 w-16 items-center justify-center rounded-xl text-2xl font-bold text-white" style="background: var(--accent)">
            {item.name.slice(0, 1).toUpperCase()}
          </div>
        {/if}
        <div class="min-w-0 flex-1">
          <h2 class="truncate text-lg font-bold">{item.name}</h2>
          <div class="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-zinc-400 dark:text-zinc-500">
            {#if isPkg}
              <span class="font-medium text-orange-600 dark:text-orange-400">{isAur ? "AUR package" : "repository package"}</span>
              {#if pkgInfo?.section && !isAur}<span>{pkgInfo.section}</span>{/if}
              {#if loading}
                <span>loading details…</span>
              {:else if pkgInfo}
                <span class="font-medium text-zinc-500 dark:text-zinc-400">
                  {pkgInfo.version}{#if pkgInfo.installedSizeKb}&nbsp;· {formatBytes(pkgInfo.installedSizeKb * 1024)} installed{/if}
                </span>
              {:else if item.version}
                <span class="font-medium text-zinc-500 dark:text-zinc-400">{item.version}</span>
              {/if}
            {:else}
              {#if item.authors?.[0]?.name}<span>by {item.authors[0].name}</span>{/if}
              {#if item.license}<span>{item.license}</span>{/if}
              {#if loading}
                <span>checking latest version…</span>
              {:else if release}
                <span class="font-medium text-zinc-500 dark:text-zinc-400">
                  v{release.version}{#if release.assets?.[0]?.size}&nbsp;· {formatBytes(release.assets[0].size)}{/if}
                </span>
              {/if}
            {/if}
          </div>
          <div class="mt-2.5 flex flex-wrap items-center gap-2">
            {#if isInstalled}
              <span class="text-sm font-medium text-green-600 dark:text-green-400">✓ Installed</span>
              <button class="btn-danger !py-1 text-xs" disabled={pkgBusy} on:click={remove}>
                {confirming ? "Really remove?" : "Remove"}
              </button>
            {:else if pkgBusy}
              <span class="text-sm text-zinc-400">Installing…</span>
            {:else if prog}
              <span class="text-sm tabular-nums text-zinc-400">
                {prog.phase === "integrating" ? "Integrating…" : pct !== null ? `Downloading ${pct}%` : "Downloading…"}
              </span>
            {:else}
              <button class="btn-primary" on:click={install}>{isAur ? "Review PKGBUILD…" : "Install"}</button>
            {/if}
            {#if isPkg && pkgInfo?.homepage}
              <button class="btn-ghost !py-1.5 text-xs" on:click={() => openUrl(pkgInfo.homepage)}>Homepage ↗</button>
            {/if}
            {#if !isPkg && item.github}
              <button class="btn-ghost !py-1.5 text-xs" on:click={() => openUrl(item.github)}>GitHub ↗</button>
            {/if}
            {#if !isPkg && item.download}
              <button class="btn-ghost !py-1.5 text-xs" on:click={() => openUrl(item.download)}>Download page ↗</button>
            {/if}
          </div>
          {#if relError}
            <p class="mt-2 text-xs text-amber-600 dark:text-amber-400">{relError}</p>
          {/if}
        </div>
        <button class="btn-ghost !px-2.5 !py-1" on:click={close}>✕</button>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto p-5">
        {#if !isPkg && item.screenshots?.length}
          <div class="mb-4 flex gap-3 overflow-x-auto pb-1.5">
            {#each item.screenshots as shot}
              <img
                src={shot}
                alt="Screenshot"
                class="h-44 shrink-0 rounded-lg border border-zinc-200 object-cover dark:border-zinc-700"
                on:error={(e) => e.currentTarget.remove()}
              />
            {/each}
          </div>
        {/if}
        <p class="whitespace-pre-line text-sm leading-relaxed text-zinc-600 dark:text-zinc-300">
          {#if isPkg}
            {pkgInfo?.description || item.plainDesc || "No description available."}
          {:else}
            {stripHtml(item.description, true) || "No description available."}
          {/if}
        </p>
        {#if !isPkg && item.categories?.length}
          <div class="mt-4 flex flex-wrap gap-1.5">
            {#each item.categories as cat}
              <span class="rounded-full bg-zinc-100 px-2.5 py-1 text-xs text-zinc-500 dark:bg-zinc-700/70 dark:text-zinc-300">{cat}</span>
            {/each}
          </div>
        {/if}
        {#if isPkg && pkgInfo?.installedVersion && pkgInfo.installedVersion !== pkgInfo.version}
          <p class="mt-4 text-xs text-zinc-400">
            Installed: {pkgInfo.installedVersion} · Available: {pkgInfo.version}
          </p>
        {/if}
      </div>
    </div>
  </div>
{/if}
