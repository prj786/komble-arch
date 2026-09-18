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
    busyPkgs,
    setPkgBusy,
    toast
  } from "../stores";
  import { refreshInstalled, refreshPkgs } from "../actions";
  import * as api from "../api";
  import { stripHtml, formatBytes } from "../utils";
  import { Dialog } from "bits-ui";
  import Icon from "./ui/Icon.svelte";
  import AppIcon from "./AppIcon.svelte";

  let release = null;
  let pkgInfo = null;
  let relError = "";
  let loading = false;
  let iconError = false;
  let confirming = false;
  let lastId = null;

  $: item = $selectedApp;
  $: isPkg = item?.kind === "pkg";
  $: isAur = isPkg && item.section === "aur";
  $: isEwe = isPkg && item.section === "ewe"; // first-party (GitHub release)
  $: prog = item && !isPkg && $progress[item.id];
  // busy state is SHARED (stores.busyPkgs) so we agree with the card behind us
  $: pkgBusy = isPkg && $busyPkgs.has(item.pkg);
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
        // pacman only knows an AUR or first-party package once it is
        // installed (-Qi); before that, the search result IS the detail.
        if ((it.section !== "aur" && it.section !== "ewe") || it.installed) {
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
      setPkgBusy(item.pkg, true);
      try {
        toast(`Installing **${item.pkg}**. You may be asked for your password…`, "info");
        if (isEwe) await api.installFirstParty(item.pkg, $settings.githubToken);
        else await api.installPackage(item.pkg);
        toast(`Installed **${item.pkg}**`, "success");
        item.installed = true;
        refreshPkgs();
      } catch (e) {
        toast(e, "error");
      }
      setPkgBusy(item.pkg, false);
      return;
    }
    try {
      await api.installFromItem(item, $settings);
      toast(`Installed **${item.name}**`, "success");
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
        setPkgBusy(item.pkg, true);
        toast(`Removing **${item.pkg}**. You may be asked for your password…`, "info");
        await api.removePackage(item.pkg);
        toast(`Removed **${item.pkg}**`, "success");
        item.installed = false;
        refreshPkgs();
        setPkgBusy(item.pkg, false);
      } else {
        await api.removeAppimage(item.id);
        toast(`Removed **${item.name}**`, "success");
        refreshInstalled();
      }
    } catch (e) {
      toast(e, "error");
      if (isPkg) setPkgBusy(item.pkg, false);
    }
  }
</script>

<!-- The app's page, as a large Dialog: bits-ui gives the focus trap, Esc,
     click-outside (it closes, as before: nothing is lost) and focus return. -->
{#if item}
  <Dialog.Root open={true} onOpenChange={(v) => !v && close()}>
    <Dialog.Portal>
      <Dialog.Overlay class="scrim" />
      <Dialog.Content class="ewe-dialog ewe-dialog--lg is-floating detail" aria-describedby={undefined}>
        <div class="ewe-dialog__head">
          <AppIcon {item} size="xl" />
          <div class="ewe-dialog__titles">
            <Dialog.Title class="ewe-dialog__title">{item.name}</Dialog.Title>
            <div class="detail__meta">
              {#if isPkg}
                <span class="ewe-badge {isAur ? 'ewe-badge--warning' : isEwe ? 'ewe-badge--accent' : ''}">
                  <span class="ewe-badge__label">{isAur ? "AUR" : isEwe ? "ewe" : pkgInfo?.section || item.section || "Repository"}</span>
                </span>
                {#if loading}
                  <span>Loading details…</span>
                {:else if pkgInfo}
                  <span class="ver">{pkgInfo.version}</span>
                  {#if pkgInfo.installedSizeKb}<span>{formatBytes(pkgInfo.installedSizeKb * 1024)} installed</span>{/if}
                {:else if item.version}
                  <span class="ver">{item.version}</span>
                {/if}
              {:else}
                <span class="ewe-badge ewe-badge--info"><span class="ewe-badge__label">AppImage</span></span>
                {#if item.authors?.[0]?.name}<span>by {item.authors[0].name}</span>{/if}
                {#if item.license}<span>{item.license}</span>{/if}
                {#if loading}
                  <span>Checking the latest version…</span>
                {:else if release}
                  <span class="ver">{release.version}</span>
                  {#if release.assets?.[0]?.size}<span>{formatBytes(release.assets[0].size)}</span>{/if}
                {/if}
              {/if}
            </div>
            <div class="detail__actions">
              {#if isInstalled}
                <span class="ewe-badge ewe-badge--success"><Icon name="check" size="xs" /><span class="ewe-badge__label">Installed</span></span>
                <!-- two steps: the first press arms it (4 s), the second removes -->
                <button class="ewe-btn ewe-btn--sm {confirming ? 'ewe-btn--danger' : 'ewe-btn--secondary'}" disabled={pkgBusy} on:click={remove}>
                  <Icon name="trash" />{confirming ? `Remove ${item.name}` : "Remove"}
                </button>
              {:else if pkgBusy || prog}
                <div
                  class="ewe-progress {pct === null ? 'ewe-progress--indeterminate' : ''} detail__progress"
                  role="progressbar"
                  aria-label="Installing {item.name}"
                  aria-valuemin="0"
                  aria-valuemax="100"
                  aria-valuenow={pct ?? undefined}
                >
                  <div class="ewe-progress__head">
                    <span class="ewe-progress__label">
                      {prog && prog.phase === "integrating" ? "Adding to the app menu…" : prog ? "Downloading…" : "Installing…"}
                    </span>
                    {#if pct !== null}<span class="ewe-progress__value">{pct}%</span>{/if}
                  </div>
                  <div class="ewe-progress__track"><div class="ewe-progress__fill" style="--value: {pct ?? 0}%"></div></div>
                </div>
              {:else}
                <button class="ewe-btn ewe-btn--primary" on:click={install}>
                  {#if isAur}<Icon name="fileCode" />Review PKGBUILD…{:else}<Icon name="download" />Install{/if}
                </button>
              {/if}
              {#if isPkg && pkgInfo?.homepage}
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => openUrl(pkgInfo.homepage)}>Homepage<Icon name="external" /></button>
              {/if}
              {#if !isPkg && item.github}
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => openUrl(item.github)}>GitHub<Icon name="external" /></button>
              {/if}
              {#if !isPkg && item.download}
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => openUrl(item.download)}>Download page<Icon name="external" /></button>
              {/if}
            </div>
            {#if relError}
              <p class="ewe-dialog__desc warn">{relError}</p>
            {/if}
          </div>
          <Dialog.Close class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Close">
            <Icon name="x" />
          </Dialog.Close>
        </div>

        <div class="detail__body">
          {#if !isPkg && item.screenshots?.length}
            <div class="shots">
              {#each item.screenshots as shot}
                <img src={shot} alt="Screenshot of {item.name}" on:error={(e) => e.currentTarget.remove()} />
              {/each}
            </div>
          {/if}
          <p class="detail__desc">
            {#if isPkg}
              {pkgInfo?.description || item.plainDesc || "No description."}
            {:else}
              {stripHtml(item.description, true) || "No description."}
            {/if}
          </p>
          {#if !isPkg && item.categories?.length}
            <div class="badges">
              {#each item.categories as cat}
                <span class="ewe-badge"><span class="ewe-badge__label">{cat}</span></span>
              {/each}
            </div>
          {/if}
          {#if isPkg && pkgInfo?.installedVersion && pkgInfo.installedVersion !== pkgInfo.version}
            <dl class="ewe-kv">
              <dt>Installed</dt><dd class="ewe-kv__mono">{pkgInfo.installedVersion}</dd>
              <dt>Available</dt><dd class="ewe-kv__mono">{pkgInfo.version}</dd>
            </dl>
          {/if}
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
{/if}
