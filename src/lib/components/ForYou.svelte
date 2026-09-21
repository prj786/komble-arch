<script>
  // "For you" — the one file's app list. ewe.conf carries [apps.installed],
  // everything Komble recorded on whichever machine wrote it; this view reads
  // it THROUGH ewe-conf and offers what is missing here. Komble never syncs,
  // fetches or restores that file (RFC-005): the account, push/pull and
  // restore live in the shell and Settings — a restore done there simply
  // shows up here on the next re-read (window focus, or every minute).
  import { onMount, onDestroy } from "svelte";
  import { route, aurReview, aurQueue, toast, settings } from "../stores";
  import { installPackage, installPackages, installFromItem } from "../api";
  import { get } from "svelte/store";
  import { refreshPkgs } from "../actions";
  import * as api from "../api";
  import sheep from "../../assets/ewe-mark-hero.svg?raw";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";
  import AppIcon from "./AppIcon.svelte";

  let manifest = null; // { apps, readAt } from [apps.installed], via ewe-conf
  let reason = ""; // why there is none: "no-manifest" | "no-ewe-conf" | …
  let loaded = false; // first read done (so the empty states can tell "loading" apart)
  let busyPkg = "";
  let skipped = 0; // first-party entries: part of ewe, never "missing"
  let installError = ""; // the last install failure, kept on screen (toasts vanish)
  let installNote = ""; // what the restore is doing right now (updates first, then apps)
  let current = false; // this session already made sure the system is current
  let timer;

  async function readManifest() {
    try {
      const m = await api.appManifest();
      if (!m.available) {
        manifest = null;
        reason = m.reason || "no-manifest";
        return;
      }
      reason = "";
      skipped = m.skipped || 0;
      const apps = [
        ...(m.packages || []).map((p) => ({
          name: p.package, installed: !!p.installed, aur: p.source === "aur",
        })),
        ...(m.appimages || []).map((a) => ({
          name: a.name || a.id, installed: !!a.installed, aur: false, appimage: true,
          id: a.id, github: a.github || null,
        })),
      ];
      manifest = { apps, readAt: Date.now() };
    } catch {
      manifest = null;
      reason = reason || "no-manifest";
    } finally {
      loaded = true;
    }
  }

  // re-read when the window comes back (a restore in Settings, an install
  // elsewhere) and once a minute regardless; never while an install runs
  const onFocus = () => { if (!busyPkg) readManifest(); };
  onMount(async () => {
    await readManifest();
    window.addEventListener("focus", onFocus);
    timer = setInterval(onFocus, 60_000);
  });
  onDestroy(() => {
    clearInterval(timer);
    window.removeEventListener("focus", onFocus);
  });

  /** A fresh install carries the sync databases of its install day; the
   *  mirrors have long moved on, so `pacman -S` of anything they list 404s.
   *  The only correct move on Arch is the full upgrade first — one prompt,
   *  once per session. */
  async function ensureCurrent() {
    if (current) return;
    let pending = [];
    try {
      pending = (await api.listUpgradable()).filter((u) => u.source === "repo");
    } catch {
      // cannot tell — go ahead; pacman's own error will say if the db is stale
    }
    if (pending.length) {
      installNote = `Updating the system first (${pending.length} update${pending.length === 1 ? "" : "s"} pending), so pacman can fetch the apps. You may be asked for your password…`;
      toast(installNote, "info", 6000);
      await api.systemUpgrade();
      refreshPkgs();
    }
    current = true;
  }

  async function install(a) {
    installError = "";
    if (a.appimage) {
      if (!a.github) {
        // no release source in the manifest (a local .AppImage install) —
        // Discover is the only road back
        route.set("discover");
        toast(`Search for “${a.name}” in Discover to install it again.`, "info", 5000);
        return;
      }
      // the manifest carries the GitHub source — reinstall through the same
      // release-resolution path Discover uses
      busyPkg = a.name;
      try {
        await installFromItem({ id: a.id, name: a.name, github: a.github }, get(settings));
        toast(`Installed **${a.name}** again`, "success");
        a.installed = true;
        manifest = manifest;
      } catch (e) {
        toast(e, "error");
      }
      busyPkg = "";
      return;
    }
    if (a.aur) {
      // AUR builds go through the PKGBUILD review gate, same as everywhere;
      // the rest of the file's AUR list queues up behind it (Review next)
      reviewAur(a.name);
      return;
    }
    busyPkg = a.name;
    try {
      await ensureCurrent();
      installNote = `Installing ${a.name}. You may be asked for your password…`;
      await installPackage(a.name);
      toast(`Installed **${a.name}**`, "success");
      a.installed = true;
      manifest = manifest;
      refreshPkgs();
    } catch (e) {
      installError = `${a.name}: ${e}`;
      toast(e, "error");
    }
    installNote = "";
    busyPkg = "";
  }

  /** Open the PKGBUILD review for one AUR app and queue the other missing
   *  AUR apps behind it — the AUR view opens the next one after each build. */
  function reviewAur(first) {
    const rest = missing.filter((x) => x.aur && x.name !== first).map((x) => x.name);
    aurQueue.set(rest);
    aurReview.set(first);
    route.set("aur");
  }

  async function installAllMissing() {
    installError = "";
    const repo = missing.filter((x) => !x.aur && !x.appimage).map((x) => x.name);
    const aurLeft = missing.filter((x) => x.aur);
    if (repo.length) {
      busyPkg = "*";
      try {
        await ensureCurrent();
        installNote = `Installing ${repo.length} app${repo.length === 1 ? "" : "s"} from the repositories in one go. You may be asked for your password…`;
        await installPackages(repo);
        toast(`Installed ${repo.length} app${repo.length === 1 ? "" : "s"}`, "success");
        for (const a of manifest.apps) if (repo.includes(a.name)) a.installed = true;
        manifest = manifest;
        refreshPkgs();
      } catch (e) {
        installError = String(e);
        toast(e, "error");
      }
      installNote = "";
      busyPkg = "";
    }
    if (aurLeft.length)
      toast(`${aurLeft.length} AUR package${aurLeft.length === 1 ? "" : "s"} left. “Review AUR apps” walks you through each PKGBUILD.`, "info", 6000);
  }

  $: missing = (manifest?.apps || []).filter((a) => !a.installed);
  $: present = (manifest?.apps || []).filter((a) => a.installed);
  $: missingAur = missing.filter((a) => a.aur);
  $: missingRepo = missing.filter((a) => !a.aur && !a.appimage);
</script>

<Page title="For you" desc="Apps your ewe.conf recorded on your other machines">
  <svelte:fragment slot="actions">
    <button class="ewe-btn ewe-btn--ghost" disabled={busyPkg !== ""} on:click={readManifest} title="Read [apps.installed] from ewe.conf again">
      <Icon name="refresh" />Read again
    </button>
  </svelte:fragment>

  {#if manifest?.apps?.length}
    {#if missing.length}
      <!-- the featured banner (App card): one per page, the page's one ask -->
      <div class="ewe-hero">
        <div class="ewe-hero__text">
          <span class="ewe-hero__kicker">From your ewe.conf</span>
          <span class="ewe-hero__title">{missing.length} app{missing.length === 1 ? "" : "s"} missing from this machine</span>
          <span class="ewe-hero__desc">
            {missingRepo.length ? `${missingRepo.length} from the repositories install${missingRepo.length === 1 ? "s" : ""} in one go` : ""}{missingRepo.length && missingAur.length ? "; " : ""}{missingAur.length ? `${missingAur.length} from the AUR wait${missingAur.length === 1 ? "s" : ""} for your review` : ""}{!missingRepo.length && !missingAur.length ? "Each one installs from its own page." : "."}
          </span>
        </div>
        {#if missingRepo.length}
          <button class="ewe-btn ewe-btn--lg" disabled={busyPkg !== ""} aria-busy={busyPkg === "*"} on:click={installAllMissing}>
            {#if busyPkg === "*"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Installing…{:else}<Icon name="download" />Install {missingRepo.length}{/if}
          </button>
        {:else if missingAur.length}
          <button class="ewe-btn ewe-btn--lg" disabled={busyPkg !== ""} on:click={() => reviewAur(missingAur[0].name)}>
            <Icon name="fileCode" />Review AUR apps
          </button>
        {/if}
        <span class="ewe-hero__sheep" aria-hidden="true">{@html sheep}</span>
      </div>
    {/if}

    {#if installNote}
      <Alert tone="info">{installNote}</Alert>
    {/if}
    {#if installError}
      <!-- the toast lives eight seconds; the reason stays here -->
      <Alert tone="danger" title="Couldn’t install" dismiss={() => (installError = "")}>
        <pre class="log">{installError}</pre>
      </Alert>
    {/if}

    <Group title="Missing from this machine · {missing.length}">
      <svelte:fragment slot="action">
        {#if missingAur.length && missingRepo.length}
          <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" disabled={busyPkg !== ""} on:click={() => reviewAur(missingAur[0].name)}>
            Review AUR apps ({missingAur.length})
          </button>
        {/if}
      </svelte:fragment>
      {#if missing.length === 0}
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="success" /></span>
          <div class="ewe-empty__title">Everything in your ewe.conf is installed here</div>
        </div>
      {:else}
        {#each missing as a (a.name)}
          <div class="ewe-row">
            <span class="ewe-row__lead"><AppIcon name={a.name} pkg={!a.appimage} size="sm" /></span>
            <div class="ewe-row__text">
              <div class="row-title">
                <span class="ewe-row__title">{a.name}</span>
                {#if a.aur}
                  <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">AUR</span></span>
                {:else if a.appimage}
                  <span class="ewe-badge ewe-badge--info"><span class="ewe-badge__label">AppImage</span></span>
                {/if}
              </div>
            </div>
            <div class="ewe-row__trail">
              {#if busyPkg === a.name || (busyPkg === "*" && !a.aur && !a.appimage)}
                <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Installing…</span>
              {:else}
                <button
                  class="ewe-btn ewe-btn--sm {a.aur ? 'ewe-btn--secondary' : 'ewe-btn--primary'}"
                  disabled={busyPkg !== ""}
                  aria-label="{a.aur ? 'Review' : 'Install'} {a.name}"
                  on:click={() => install(a)}
                >
                  {a.aur ? "Review…" : "Install"}
                </button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </Group>

    <Group title="Already installed · {present.length}">
      <div class="badges badges--well">
        {#each present as a (a.name)}
          <span class="ewe-badge"><span class="ewe-badge__label">{a.name}</span></span>
        {/each}
        {#if skipped}
          <span class="ewe-badge ewe-badge--accent" title="Komble, ewe-settings and ewe-sync come with the desktop">
            <span class="ewe-badge__label">+{skipped} part of ewe</span>
          </span>
        {/if}
      </div>
    </Group>
  {:else if !loaded}
    <div class="ewe-empty" aria-busy="true">
      <span class="ewe-empty__icon"><span class="ewe-spinner ewe-spinner--xl" aria-hidden="true"></span></span>
      <div class="ewe-empty__title">Reading your ewe.conf…</div>
    </div>
  {:else if reason === "no-ewe-conf"}
    <div class="ewe-empty">
      <span class="ewe-empty__icon"><Icon name="alert" /></span>
      <div class="ewe-empty__title">No app list to read</div>
      <div class="ewe-empty__desc">ewe-conf isn’t installed here. For you needs the ewe desktop, 0.9 or newer.</div>
    </div>
  {:else if manifest && manifest.apps.length === 0}
    <div class="ewe-empty">
      <span class="ewe-empty__icon"><Icon name="package" /></span>
      <div class="ewe-empty__title">No apps recorded yet</div>
      <div class="ewe-empty__desc">Every app you install with Komble from now on is recorded in your ewe.conf.</div>
    </div>
  {:else}
    <!-- no [apps.installed] at all: a fresh file, or an old backup that never
         carried one. Restoring is not Komble's job — say where it lives. -->
    <div class="ewe-empty">
      <span class="ewe-empty__icon"><Icon name="history" /></span>
      <div class="ewe-empty__title">Your ewe.conf has no app list yet</div>
      <div class="ewe-empty__desc">
        Every app you install here is recorded from now on. To bring the apps from another ewe machine, restore its file in Settings › Account. The list then appears here by itself.
      </div>
    </div>
  {/if}

  <p class="note">
    Only apps are listed: packages that ship a launcher. Kernels, drivers and libraries come back the usual way, as dependencies.
  </p>
</Page>
