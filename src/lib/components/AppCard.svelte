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
  import AppIcon from "./AppIcon.svelte";

  export let item;

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
  // App card, "Sources": repository names are a neutral Badge, the AUR is
  // warning (built from source), AppImages are info
  $: source = isAur
    ? { tone: "warning", label: "AUR" }
    : isPkg
      ? { tone: isEwe ? "accent" : "", label: item.section || "pacman" }
      : { tone: "info", label: "AppImage" };
  $: meta = isPkg ? item.version || "" : (item.categories || []).slice(0, 2).join(" · ");
  $: dev = isPkg ? (isAur ? "Arch User Repository" : isEwe ? "ewe" : "Arch package") : item.authors?.[0]?.name || "";

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
        toast(`Installing **${item.pkg}**. You may be asked for your password…`, "info");
        if (isEwe) await installFirstParty(item.pkg, $settings.githubToken);
        else await installPackage(item.pkg);
        toast(`Installed **${item.pkg}**`, "success");
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
      toast(`Installed **${item.name}**`, "success");
    } catch (err) {
      toast(err, "error");
    }
  }
</script>

<!-- App card (design/system/components/AppCard): the whole card opens the
     app's page and is named by the app; its button is a separate control. -->
<div
  role="button"
  tabindex="0"
  aria-label={item.name}
  class="ewe-appcard"
  on:click={() => selectedApp.set(item)}
  on:keydown={(e) => (e.key === "Enter" || e.key === " ") && e.target === e.currentTarget && (e.preventDefault(), selectedApp.set(item))}
>
  <div class="ewe-appcard__top">
    <AppIcon {item} />
    <div class="ewe-appcard__titles">
      <span class="ewe-appcard__name">{item.name}</span>
      {#if dev}<span class="ewe-appcard__dev">{dev}</span>{/if}
    </div>
  </div>

  <div class="ewe-appcard__summary">{item.plainDesc || "No description."}</div>

  <div class="ewe-appcard__foot">
    {#if prog || pkgBusy}
      <!-- Installing: a Progress bar with its label -->
      <div
        class="ewe-progress {pct === null ? 'ewe-progress--indeterminate' : ''}"
        role="progressbar"
        aria-label="Installing {item.name}"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={pct ?? undefined}
      >
        <div class="ewe-progress__head">
          <span class="ewe-progress__label">
            {prog && prog.phase === "integrating" ? "Adding to the app menu…" : pct !== null ? `Installing… ${pct}%` : "Installing…"}
          </span>
        </div>
        <div class="ewe-progress__track"><div class="ewe-progress__fill" style="--value: {pct ?? 0}%"></div></div>
      </div>
    {:else}
      <span class="ewe-meta">
        <span class="ewe-badge {source.tone ? `ewe-badge--${source.tone}` : ''}"><span class="ewe-badge__label">{source.label}</span></span>
        {#if meta}<span>{meta}</span>{/if}
      </span>
      {#if isInstalled}
        <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Installed</span></span>
      {:else}
        <button
          class="ewe-btn ewe-btn--sm ewe-btn--primary"
          aria-label="{isAur ? 'Review' : 'Install'} {item.name}"
          on:click={install}
          on:keydown|stopPropagation>{isAur ? "Review…" : "Install"}</button
        >
      {/if}
    {/if}
  </div>
</div>
