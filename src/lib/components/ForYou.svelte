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
      installNote = `${pending.length} update${pending.length === 1 ? "" : "s"} pending — bringing the system up to date first, so pacman can fetch the apps (authentication may be required)…`;
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
        toast(`Search for "${a.name}" in Discover to reinstall it.`, "info", 5000);
        return;
      }
      // the manifest carries the GitHub source — reinstall through the same
      // release-resolution path Discover uses
      busyPkg = a.name;
      try {
        await installFromItem({ id: a.id, name: a.name, github: a.github }, get(settings));
        toast(`${a.name} reinstalled`, "success");
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
      installNote = `Installing ${a.name} — authentication may be required…`;
      await installPackage(a.name);
      toast(`${a.name} installed`, "success");
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
        installNote = `Installing ${repo.length} app${repo.length === 1 ? "" : "s"} from the repositories in one go — authentication may be required…`;
        await installPackages(repo);
        toast(`${repo.length} app${repo.length === 1 ? "" : "s"} installed`, "success");
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
      toast(`${aurLeft.length} AUR package${aurLeft.length === 1 ? "" : "s"} left — “Review AUR apps” walks you through each PKGBUILD.`, "info", 6000);
  }

  $: missing = (manifest?.apps || []).filter((a) => !a.installed);
  $: present = (manifest?.apps || []).filter((a) => a.installed);
  $: missingAur = missing.filter((a) => a.aur);
  $: missingRepo = missing.filter((a) => !a.aur && !a.appimage);
</script>

<div class="h-full overflow-y-auto px-4 py-5 sm:px-6 sm:py-6">
  <div class="mb-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
    <div>
      <h1 class="text-xl font-bold tracking-tight">For you</h1>
      <p class="text-sm text-dim dark:text-dim">Apps from your ewe.conf — installed on another machine, recorded in the one file</p>
    </div>
    <button class="btn-ghost !py-1 text-xs" disabled={busyPkg !== ""} on:click={readManifest} title="Re-read [apps.installed] from ewe.conf">
      Re-read
    </button>
  </div>

  {#if manifest?.apps?.length}
    <div class="section-title">Apps missing from this machine · {missing.length}</div>
    {#if missing.length === 0}
      <div class="card p-5 text-center text-sm text-dim">
        Everything in your ewe.conf is installed here ✓
      </div>
    {:else}
      <div class="mb-2 flex flex-wrap items-center justify-end gap-2">
        {#if missingAur.length}
          <button class="btn-ghost !py-1 text-xs" disabled={busyPkg !== ""} on:click={() => reviewAur(missingAur[0].name)}>
            Review AUR apps ({missingAur.length})
          </button>
        {/if}
        {#if missingRepo.length}
          <button class="btn-ghost !py-1 text-xs" disabled={busyPkg !== ""} on:click={installAllMissing}>
            {busyPkg === "*" ? "Installing…" : `Install all repo apps (${missingRepo.length})`}
          </button>
        {/if}
      </div>
      {#if installNote}
        <div class="mb-2 rounded-lg border border-[var(--link)] bg-[color-mix(in_srgb,var(--link)_14%,transparent)]0/5 px-3 py-2 text-xs text-link dark:text-link">{installNote}</div>
      {/if}
      {#if installError}
        <!-- the toast lives eight seconds; the reason stays here -->
        <div class="mb-2 rounded-lg border border-[var(--danger)] bg-[color-mix(in_srgb,var(--danger)_14%,transparent)]0/5 px-3 py-2 text-xs text-danger dark:text-danger">
          <div class="flex items-center justify-between gap-2">
            <span class="font-medium">Install failed</span>
            <button class="btn-ghost !py-0.5 text-[11px]" on:click={() => (installError = "")}>Dismiss</button>
          </div>
          <pre class="mt-1 max-h-40 overflow-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed">{installError}</pre>
        </div>
      {/if}
      <div class="flex flex-col gap-2">
        {#each missing as a (a.name)}
          <div class="card flex items-center gap-3.5 px-4 py-3">
            <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-elevated text-sm font-bold text-dim">
              {a.name.slice(0, 1).toUpperCase()}
            </div>
            <div class="min-w-0 flex-1">
              <span class="truncate font-medium">{a.name}</span>
              {#if a.aur}
                <span class="ml-2 rounded-full bg-orange-500/15 px-2 py-0.5 text-[11px] font-medium text-orange-600 dark:text-orange-400">AUR</span>
              {/if}
            </div>
            {#if busyPkg === a.name || (busyPkg === "*" && !a.aur && !a.appimage)}
              <span class="text-xs text-dim">Installing…</span>
            {:else}
              <button class="btn-primary !py-1 text-xs" disabled={busyPkg !== ""} on:click={() => install(a)}>
                {a.aur ? "Review…" : "Install"}
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    <div class="section-title">Already installed · {present.length}</div>
    <div class="card flex flex-wrap gap-1.5 p-3">
      {#each present as a (a.name)}
        <span class="rounded-full bg-elevated px-2.5 py-1 text-xs text-dim /70 ">{a.name}</span>
      {/each}
      {#if skipped}
        <span class="rounded-full bg-elevated/60 px-2.5 py-1 text-xs text-dim  dark:text-dim" title="Komble, ewe-settings and ewe-sync come with the desktop">
          + {skipped} part of ewe
        </span>
      {/if}
    </div>
  {:else if !loaded}
    <div class="card p-6 text-center text-sm text-dim">Reading your ewe.conf…</div>
  {:else if reason === "no-ewe-conf"}
    <div class="card p-6 text-center text-sm text-dim">
      <span class="text-muted">ewe-conf</span> is not installed here, so there is no
      app list to read — this needs the ewe desktop (0.9 or newer).
    </div>
  {:else if manifest && manifest.apps.length === 0}
    <div class="card p-6 text-center text-sm text-dim">
      The app list in your ewe.conf is empty — nothing Komble knows about has
      been installed yet. Every install from here on is recorded there.
    </div>
  {:else}
    <!-- no [apps.installed] at all: a fresh file, or an old backup that never
         carried one. Restoring is not Komble's job — say where it lives. -->
    <div class="card p-6 text-center text-sm text-dim">
      Your ewe.conf has no app list yet. Every app you install here is recorded
      in it from now on. To bring the apps from another ewe machine, restore
      that machine's file from
      <span class="text-muted">Settings → Account</span> — the list appears
      here by itself.
    </div>
  {/if}

  <p class="mt-4 text-xs text-dim dark:text-dim">
    Only applications are listed — packages that ship a launcher. Kernels, drivers and
    libraries are restored the usual way (dependencies come along automatically).
  </p>
</div>
