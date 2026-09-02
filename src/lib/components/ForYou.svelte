<script>
  // "For you" — the DE-account view. Komble is THE source of truth for apps,
  // so restoring your applications from the Google backup lives HERE (the
  // Settings app keeps the settings side). The Google account itself is owned
  // by the shell; this view only sends allowlisted `qs ipc` verbs and reads
  // the package cache the shell writes — tokens never enter Komble.
  import { onMount, onDestroy } from "svelte";
  import { route, aurReview, aurQueue, trackedPkgs, toast, settings } from "../stores";
  import { installPackage, installPackages, installFromItem } from "../api";
  import { get } from "svelte/store";
  import { refreshPkgs } from "../actions";
  import * as api from "../api";

  let google = null; // null = shell unreachable
  let backup = null; // { apps, fetchedAt } from the local [apps.installed] manifest
  let backupReason = ""; // why there is no manifest: "no-manifest" | "no-ewe-conf" | …
  let fetched = false; // a pull succeeded in this session — the file IS the backup now
  let cloud = null; // { device, updatedAt } — the Drive copy's facts (status probe only)
  let checking = false;
  let syncing = false;
  let busyPkg = "";
  let skipped = 0; // first-party entries (Komble, ewe-settings): part of ewe, never "missing"
  let installError = ""; // the last install failure, kept on screen (toasts vanish)
  let installNote = ""; // what the restore is doing right now (updates first, then apps)
  let current = false; // this session already made sure the system is current
  let timer;

  async function refreshStatus() {
    try {
      const raw = (await api.qsIpc("status")).trim();
      google = raw && raw !== "null" ? JSON.parse(raw) : null;
    } catch {
      google = null;
    }
    if (google?.syncState !== "syncing") syncing = false;
  }

  // RFC-002: the app list comes from ewe.conf's [apps.installed] manifest
  // (read through ewe-conf), shaped like the legacy bundle so the UI below
  // is unchanged. `aur` steers the install path to the PKGBUILD review gate.
  async function readBackup() {
    try {
      const m = await api.restoreManifest();
      if (!m.available) {
        backup = null;
        backupReason = m.reason || "no-manifest";
        return;
      }
      backupReason = "";
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
      backup = { apps, fetchedAt: Date.now() };
    } catch {
      backup = null;
    }
  }

  onMount(async () => {
    await refreshStatus();
    await readBackup();
    if (google?.signedIn) probeCloud();
    timer = setInterval(() => {
      if (syncing || google?.busy) refreshStatus();
    }, 2000);
  });
  onDestroy(() => clearInterval(timer));

  /** Facts only — never touches the local file. (This used to be a silent
   *  pull that overwrote ewe.conf the moment the pane opened.) */
  async function probeCloud() {
    try {
      const st = await api.confSyncStatus();
      cloud =
        st.ok && st.remote
          ? {
              device: st.remote.appProperties?.machine || "your account",
              updatedAt: st.remote.modifiedTime || ""
            }
          : null;
    } catch {
      cloud = null;
    }
  }

  /** EXPLICIT restore: pull the one file from Drive (ewe-conf keeps a
   *  timestamped backup of the old one), apply it, re-read the manifest. */
  async function fetchBackup() {
    checking = true;
    try {
      const r = await api.confSync("pull");
      if (r.ok === false && r.error === "not-signed-in") {
        checking = false;
        return;
      }
      if (r.ok === false && r.error !== "nothing-synced") toast(r.error, "error");
      if (r.ok) {
        fetched = true;
        toast("Backup fetched — the previous local file is kept as a .bak.", "success");
      }
      await readBackup();
      await probeCloud();
    } catch (e) {
      toast(e, "error");
    }
    checking = false;
  }

  async function syncNow() {
    syncing = true;
    try {
      const r = await api.confSync("push");
      if (r.ok === false) throw pushError(r.error);
      toast("Your machine's file is synced to Drive.", "success");
    } catch (e) {
      toast(e, "error");
      syncing = false;
    }
    setTimeout(refreshStatus, 1500);
  }

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
        backup = backup;
      } catch (e) {
        toast(e, "error");
      }
      busyPkg = "";
      return;
    }
    if (a.aur) {
      // AUR builds go through the PKGBUILD review gate, same as everywhere;
      // the rest of the backup's AUR list queues up behind it (Review next)
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
      backup = backup;
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
        for (const a of backup.apps) if (repo.includes(a.name)) a.installed = true;
        backup = backup;
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

  // ewe-conf's push guard codes → words a person can act on
  const pushError = (code) =>
    ({
      "remote-exists":
        "A backup already exists on Drive from another machine — restore it first (Fetch backup), or force the push from Settings → Account.",
      "remote-newer":
        "Another machine saved newer settings — fetch the backup first, or force the push from Settings → Account.",
      "not-signed-in": "Not signed in — connect a Google account in Settings → Account.",
    })[code] || code;

  $: missing = (backup?.apps || []).filter((a) => !a.installed);
  $: present = (backup?.apps || []).filter((a) => a.installed);
  $: missingAur = missing.filter((a) => a.aur);
  $: missingRepo = missing.filter((a) => !a.aur && !a.appimage);
  const fmt = (iso) => {
    if (!iso) return "";
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  };
</script>

<div class="h-full overflow-y-auto px-4 py-5 sm:px-6 sm:py-6">
  <div class="mb-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
    <div>
      <h1 class="text-xl font-bold tracking-tight">For you</h1>
      <p class="text-sm text-zinc-400 dark:text-zinc-500">Your apps, synced with your Google account</p>
    </div>
    {#if google?.signedIn}
      <div class="flex gap-2">
        <button class="btn-ghost" disabled={checking} on:click={fetchBackup}
                title="Downloads your Drive backup over this machine's file (the old one is kept as a .bak)">
          {checking ? "Fetching…" : "Fetch backup"}
        </button>
        <button class="btn-primary" disabled={syncing || google.syncState === "syncing"} on:click={syncNow}>
          {syncing || google.syncState === "syncing" ? "Syncing…" : "Sync now"}
        </button>
      </div>
    {/if}
  </div>

  {#if google === null}
    <div class="card p-6 text-center text-sm text-zinc-400">
      The ewe session is not reachable — the Google account lives in the shell.
    </div>
  {:else if !google.signedIn}
    <!-- freedom by absence: no promo, no button — the one sign-in lives in
         Settings → Account. This pane just says where. -->
    <p class="text-sm text-zinc-500">
      Not connected. To sync your apps between machines, connect a Google
      account in <span class="text-zinc-300">Settings → Account</span>.
    </p>
  {:else}
    <div class="card mb-4 flex items-center gap-3 px-4 py-3">
      {#if google.profile?.picture}
        <img src={google.profile.picture} alt="" class="h-10 w-10 rounded-full" />
      {/if}
      <div class="min-w-0 flex-1">
        <div class="truncate font-medium">{google.profile?.name || "Google"}</div>
        <div class="truncate text-xs text-zinc-400">
          {google.profile?.email || ""}
          {#if google.lastSync}<span class="mx-1">·</span>last sync {fmt(google.lastSync)}{/if}
          {#if google.autoSync}<span class="mx-1">·</span>auto-sync on — installs and settings upload by themselves{/if}
        </div>
      </div>
      {#if google.syncError}
        <span class="text-xs text-amber-500">{google.syncError}</span>
      {/if}
    </div>

    {#if backup?.apps?.length}
      <div class="section-title">
        Apps missing from this machine · {missing.length}
        {#if cloud}<span class="ml-2 font-normal normal-case tracking-normal">cloud copy from {cloud.device}, {fmt(cloud.updatedAt)}</span>{/if}
      </div>
      {#if missing.length === 0}
        <div class="card p-5 text-center text-sm text-zinc-400">
          Everything from your backup is installed here ✓
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
          <div class="mb-2 rounded-lg border border-sky-400/40 bg-sky-500/5 px-3 py-2 text-xs text-sky-700 dark:text-sky-300">{installNote}</div>
        {/if}
        {#if installError}
          <!-- the toast lives eight seconds; the reason stays here -->
          <div class="mb-2 rounded-lg border border-red-400/40 bg-red-500/5 px-3 py-2 text-xs text-red-700 dark:text-red-300">
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
              <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-zinc-200 text-sm font-bold text-zinc-500 dark:bg-zinc-700 dark:text-zinc-300">
                {a.name.slice(0, 1).toUpperCase()}
              </div>
              <div class="min-w-0 flex-1">
                <span class="truncate font-medium">{a.name}</span>
                {#if a.aur}
                  <span class="ml-2 rounded-full bg-orange-500/15 px-2 py-0.5 text-[11px] font-medium text-orange-600 dark:text-orange-400">AUR</span>
                {/if}
              </div>
              {#if busyPkg === a.name || (busyPkg === "*" && !a.aur && !a.appimage)}
                <span class="text-xs text-zinc-400">Installing…</span>
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
          <span class="rounded-full bg-zinc-100 px-2.5 py-1 text-xs text-zinc-500 dark:bg-zinc-700/70 dark:text-zinc-300">{a.name}</span>
        {/each}
        {#if skipped}
          <span class="rounded-full bg-zinc-100/60 px-2.5 py-1 text-xs text-zinc-400 dark:bg-zinc-800/60 dark:text-zinc-500" title="Komble and ewe-settings come with the desktop">
            + {skipped} part of ewe
          </span>
        {/if}
      </div>
    {:else if checking}
      <div class="card p-6 text-center text-sm text-zinc-400">Fetching your backup from Drive…</div>
    {:else if backupReason === "no-ewe-conf"}
      <div class="card p-6 text-center text-sm text-zinc-400">
        <span class="text-zinc-300">ewe-conf</span> is not installed here, so there is no
        app list to read — this needs the ewe desktop (0.9 or newer).
      </div>
    {:else if backup && backup.apps.length === 0}
      <div class="card p-6 text-center text-sm text-zinc-400">
        This backup's app list is empty — the machine that wrote it had no apps
        Komble knew about yet.
      </div>
    {:else if backupReason && (fetched || google.lastSync)}
      <!-- the file has been restored/synced, yet carries no [apps.installed]:
           say so, instead of an empty pane that looks like a bug -->
      <div class="card p-6 text-center text-sm text-zinc-400">
        This backup has no app list. Apps installed on the other machine before
        Komble 0.9.3, or with pacman directly, were not recorded; newer ewe
        versions record every explicitly installed package, so the next sync from
        that machine will carry them.
      </div>
    {:else if cloud}
      <div class="card p-6 text-center text-sm text-zinc-400">
        A backup from <span class="text-zinc-300">{cloud.device}</span> ({fmt(cloud.updatedAt)})
        is in your Drive — “Fetch backup” brings it onto this machine.
      </div>
    {:else}
      <div class="card p-6 text-center text-sm text-zinc-400">
        No backup found yet — "Sync now" pushes this machine's apps and settings to your Drive.
      </div>
    {/if}

    <p class="mt-4 text-xs text-zinc-400 dark:text-zinc-500">
      Only applications are listed — packages that ship a launcher. Kernels, drivers and
      libraries are restored the usual way (dependencies come along automatically).
    </p>
  {/if}
</div>
