<script>
  // A curated "Popular" card (Discover). It shows one curated app and reuses
  // the exact install paths the rest of the app does — a repo package via
  // installPackage, an AUR package through the PKGBUILD review gate, an
  // AppImage via installFromItem — so there is no second install code path to
  // drift. The whole card opens the app's page (AppDetail), like AppCard.
  import {
    installedIds,
    progress,
    route,
    selectedApp,
    settings,
    trackedPkgs,
    installedPkgNames,
    aurReview,
    busyPkgs,
    setPkgBusy,
    toast
  } from "../stores";
  import { installFromItem, installPackage } from "../api";
  import { refreshPkgs } from "../actions";
  import { toItem } from "../curated";
  import AppIcon from "./AppIcon.svelte";

  export let app;

  $: item = toItem(app);
  $: isPkg = app.source !== "appimage";
  $: isAur = app.source === "aur";
  $: prog = !isPkg && $progress[item.id];
  // busy state is SHARED (stores.busyPkgs) so the detail modal agrees with us
  $: pkgBusy = isPkg && $busyPkgs.has(app.pkg);
  // installed = pacman has it, however it got there (not only through Komble)
  $: isInstalled = isPkg
    ? $installedPkgNames.has(app.pkg) || $trackedPkgs.some((d) => d.package === app.pkg)
    : $installedIds.has(item.id);
  // source tag: Repository is neutral, the AUR is warning (built from source),
  // AppImages are info — same tones as AppCard.
  $: source = isAur
    ? { tone: "warning", label: "AUR" }
    : isPkg
      ? { tone: "", label: "Repository" }
      : { tone: "info", label: "AppImage" };

  async function install(e) {
    e.stopPropagation();
    if (prog || isInstalled || pkgBusy) return;
    // AUR packages build from a user-reviewed PKGBUILD, never one-click —
    // hand over to the AUR view with the review already open.
    if (isAur) {
      aurReview.set(app.pkg);
      route.set("aur");
      return;
    }
    if (isPkg) {
      setPkgBusy(app.pkg, true);
      try {
        toast(`Installing **${app.pkg}**. You may be asked for your password…`, "info");
        await installPackage(app.pkg);
        toast(`Installed **${app.pkg}**`, "success");
        refreshPkgs();
      } catch (err) {
        toast(err, "error");
      }
      setPkgBusy(app.pkg, false);
      return;
    }
    try {
      await installFromItem(item, $settings);
      toast(`Installed **${app.name}**`, "success");
    } catch (err) {
      toast(err, "error");
    }
  }
</script>

<!-- Curated card: the App card anatomy, named by the app; its button is a
     separate control, and the card opens the app's page. -->
<div
  role="button"
  tabindex="0"
  aria-label={app.name}
  class="ewe-appcard"
  on:click={() => selectedApp.set(item)}
  on:keydown={(e) => (e.key === "Enter" || e.key === " ") && e.target === e.currentTarget && (e.preventDefault(), selectedApp.set(item))}
>
  <div class="ewe-appcard__top">
    <AppIcon {item} />
    <div class="ewe-appcard__titles">
      <span class="ewe-appcard__name">{app.name}</span>
    </div>
  </div>

  <div class="ewe-appcard__summary">{app.desc}</div>
  {#if app.note}
    <div class="curated-card__note">{app.note}</div>
  {/if}

  <div class="ewe-appcard__foot">
    <span class="ewe-meta">
      <span class="ewe-badge {source.tone ? `ewe-badge--${source.tone}` : ''}"><span class="ewe-badge__label">{source.label}</span></span>
    </span>
    {#if prog || pkgBusy}
      <!-- Installing: a busy label, no percent (a repo install has no byte
           progress; an AppImage one does, but a short label reads better) -->
      <span class="busy" role="status"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Installing…</span>
    {:else if isInstalled}
      <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Installed</span></span>
    {:else}
      <button
        class="ewe-btn ewe-btn--sm ewe-btn--primary"
        aria-label="{isAur ? 'Review' : 'Install'} {app.name}"
        on:click={install}
        on:keydown|stopPropagation>{isAur ? "Review…" : "Install"}</button
      >
    {/if}
  </div>
</div>
