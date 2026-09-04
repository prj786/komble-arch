<script>
  import {
    installedIds,
    progress,
    route,
    selectedApp,
    settings,
    trackedPkgs,
    aurReview,
    busyPkgs,
    setPkgBusy,
    toast
  } from "../stores";
  import { installFromItem, installPackage, installFirstParty } from "../api";
  import { refreshPkgs } from "../actions";

  export let item;
  let iconError = false;

  $: isPkg = item.kind === "pkg";
  $: isAur = isPkg && item.section === "aur";
  $: isEwe = isPkg && item.section === "ewe"; // first-party (GitHub release)
  $: prog = !isPkg && $progress[item.id];
  // busy state is SHARED (stores.busyPkgs) so the detail modal agrees with us
  $: pkgBusy = isPkg && $busyPkgs.has(item.pkg);
  $: isInstalled = isPkg
    ? item.installed || $trackedPkgs.some((d) => d.package === item.pkg)
    : $installedIds.has(item.id);
  $: pct = prog && prog.total > 0 ? Math.min(100, Math.round((prog.downloaded / prog.total) * 100)) : null;

  async function install(e) {
    e.stopPropagation();
    if (prog || isInstalled || pkgBusy) return;
    // AUR packages build from a user-reviewed PKGBUILD, never one-click —
    // hand over to the AUR view with the review already open.
    if (isAur) {
      aurReview.set(item.pkg);
      route.set("aur");
      return;
    }
    if (isPkg) {
      setPkgBusy(item.pkg, true);
      try {
        toast(`Installing ${item.pkg} — authentication may be required…`, "info");
        if (isEwe) await installFirstParty(item.pkg, $settings.githubToken);
        else await installPackage(item.pkg);
        toast(`${item.pkg} installed`, "success");
        item.installed = true;
        refreshPkgs();
      } catch (err) {
        toast(err, "error");
      }
      setPkgBusy(item.pkg, false);
      return;
    }
    try {
      await installFromItem(item, $settings);
      toast(`${item.name} installed`, "success");
    } catch (err) {
      toast(err, "error");
    }
  }
</script>

<div
  role="button"
  tabindex="0"
  class="card relative flex h-full w-full cursor-pointer flex-col overflow-hidden p-4 text-left transition-shadow hover:shadow-md dark:hover:border-hairline"
  on:click={() => selectedApp.set(item)}
  on:keydown={(e) => e.key === "Enter" && selectedApp.set(item)}
>
  <div class="flex items-start gap-3">
    {#if isPkg}
      <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-elevated text-dim">
        <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 7l-8-4-8 4v10l8 4 8-4V7zM4 7l8 4m0 0l8-4m-8 4v10" />
        </svg>
      </div>
    {:else if item.icon && !iconError}
      <img
        src={item.icon}
        alt=""
        loading="lazy"
        class="h-11 w-11 shrink-0 rounded-lg object-contain"
        on:error={() => (iconError = true)}
      />
    {:else}
      <div
        class="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg text-lg font-bold text-white"
        style="background: var(--accent)"
      >
        {item.name.slice(0, 1).toUpperCase()}
      </div>
    {/if}
    <div class="min-w-0">
      <div class="truncate font-semibold">{item.name}</div>
      {#if isPkg}
        <div class="truncate text-xs text-dim dark:text-dim">
          {item.section || "Arch package"}{item.version ? ` · ${item.version}` : ""}
        </div>
      {:else if item.authors?.[0]?.name}
        <div class="truncate text-xs text-dim dark:text-dim">
          {item.authors[0].name}
        </div>
      {/if}
    </div>
  </div>

  <p class="mt-2 line-clamp-2 flex-1 text-[13px] leading-snug text-dim">
    {item.plainDesc || "No description."}
  </p>

  <div class="mt-2 flex items-center gap-1.5">
    {#if isAur}
      <span class="rounded-full bg-orange-500/15 px-2 py-0.5 text-[11px] font-medium text-orange-600 dark:text-orange-400">
        AUR
      </span>
    {:else if isPkg}
      <span class="rounded-full bg-[color-mix(in_srgb,var(--brand-fg-link)_14%,transparent)]0/15 px-2 py-0.5 text-[11px] font-medium text-link dark:text-link">
        {item.section || "pacman"}
      </span>
    {:else}
      {#each (item.categories || []).slice(0, 2) as cat}
        <span class="rounded-full bg-elevated px-2 py-0.5 text-[11px] text-dim /70 ">
          {cat}
        </span>
      {/each}
    {/if}
    <span class="flex-1"></span>
    {#if isInstalled}
      <span class="text-xs font-medium text-success dark:text-success">✓ Installed</span>
    {:else if pkgBusy}
      <span class="text-xs text-dim">Installing…</span>
    {:else if prog}
      <span class="text-xs tabular-nums text-dim">
        {prog.phase === "integrating" ? "Integrating…" : pct !== null ? `${pct}%` : "…"}
      </span>
    {:else}
      <button class="btn-primary !py-1 text-xs" on:click={install}>{isAur ? "Review…" : "Install"}</button>
    {/if}
  </div>

  {#if prog}
    <div class="absolute inset-x-0 bottom-0 h-0.5 bg-elevated">
      <div
        class="h-full transition-all {pct === null ? 'w-full animate-pulse' : ''}"
        style="background: var(--accent); {pct !== null ? `width:${pct}%` : ''}"
      ></div>
    </div>
  {/if}
</div>
