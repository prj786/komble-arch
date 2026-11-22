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
    eepBusy,
    toast
  } from "../stores";
  import { installFromItem, installPackage, installFirstParty } from "../api";
  import { refreshPkgs, eepRun } from "../actions";
  import { formatBytes } from "../utils";
  import AppIcon from "./AppIcon.svelte";

  export let item;

  $: isPkg = item.kind === "pkg";
  $: isAur = isPkg && item.section === "aur";
  $: isEwe = isPkg && item.section === "ewe"; // first-party (GitHub release)
  $: isEep = item.kind === "eep"; // eep.rs: id "eep:<id>", installed = its version
  $: prog = !isPkg && $progress[item.id];
  // busy state is SHARED (stores.busyPkgs) so the detail modal agrees with us
  $: eepVerb = isEep && $eepBusy[item.id];
  $: pkgBusy = isPkg ? $busyPkgs.has(item.pkg) : !!eepVerb;
  $: isInstalled = isPkg
    ? item.installed || $trackedPkgs.some((d) => d.package === item.pkg)
    : isEep
      ? item.installed != null
      : $installedIds.has(item.id);
  $: doing = eepVerb === "remove" ? "Removing" : eepVerb === "update" ? "Updating" : "Installing";
  $: pct = prog && prog.total > 0 ? Math.min(100, Math.round((prog.downloaded / prog.total) * 100)) : null;
  // App card, "Sources": repository names are a neutral Badge, the AUR is
  // warning (built from source), AppImages are info, eep (ewe's own) accent
  $: source = isAur
    ? { tone: "warning", label: "AUR" }
    : isPkg
      ? { tone: isEwe ? "accent" : "", label: item.section || "pacman" }
      : isEep
        ? { tone: "accent", label: "eep" }
        : { tone: "info", label: "AppImage" };
  $: meta = isPkg
    ? item.version || ""
    : isEep
      ? [item.version, formatBytes(item.downloadSize)].filter(Boolean).join(" · ")
      : (item.categories || []).slice(0, 2).join(" · ");
  $: dev = isPkg
    ? isAur ? "Arch User Repository" : isEwe ? "ewe" : "Arch package"
    : isEep
      ? item.appKind === "cli" ? "Command-line tool" : item.appKind === "tool" ? "Tool" : ""
      : item.authors?.[0]?.name || "";

  async function install(e) {
    e.stopPropagation();
    // eep: an update is the one action an installed card still offers
    if (isEep) {
      if (!prog && !pkgBusy && !item.unavailable && (!isInstalled || item.update)) {
        eepRun(isInstalled ? "update" : "install", [item]);
      }
      return;
    }
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
        toast(`Installing **${item.name}**. You may be asked for your password…`, "info");
        if (isEwe) await installFirstParty(item.pkg, $settings.githubToken);
        else await installPackage(item.pkg);
        toast(`Installed **${item.name}**`, "success");
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
        aria-label="{doing} {item.name}"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={pct ?? undefined}
      >
        <div class="ewe-progress__head">
          <span class="ewe-progress__label">
            {prog && prog.phase === "integrating" ? "Adding to the app menu…" : pct !== null ? `${doing}… ${pct}%` : `${doing}…`}
          </span>
        </div>
        <div class="ewe-progress__track"><div class="ewe-progress__fill" style="--value: {pct ?? 0}%"></div></div>
      </div>
    {:else}
      <span class="ewe-meta">
        <span class="ewe-badge {source.tone ? `ewe-badge--${source.tone}` : ''}"><span class="ewe-badge__label">{source.label}</span></span>
        {#if meta}<span>{meta}</span>{/if}
      </span>
      {#if isEep && isInstalled && item.update}
        <button
          class="ewe-btn ewe-btn--sm ewe-btn--secondary"
          aria-label="Update {item.name}"
          on:click={install}
          on:keydown|stopPropagation>Update</button
        >
      {:else if isInstalled}
        <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Installed</span></span>
      {:else if isEep && item.unavailable}
        <span class="ewe-badge" title={item.unavailable}><span class="ewe-badge__label">Unavailable</span></span>
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
