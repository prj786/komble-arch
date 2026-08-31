<script>
  // "For you" — the DE-account view. Komble is THE source of truth for apps,
  // so restoring your applications from the Google backup lives HERE (the
  // Settings app keeps the settings side). The Google account itself is owned
  // by the shell; this view only sends allowlisted `qs ipc` verbs and reads
  // the package cache the shell writes — tokens never enter Komble.
  import { onMount, onDestroy } from "svelte";
  import { route, aurReview, trackedPkgs, toast } from "../stores";
  import { installPackage } from "../api";
  import { refreshPkgs } from "../actions";
  import * as api from "../api";

  let google = null; // null = shell unreachable
  let backup = null; // { apps, fetchedAt } from the local [apps.installed] manifest
  let cloud = null; // { device, updatedAt } — the Drive copy's facts (status probe only)
  let checking = false;
  let syncing = false;
  let busyPkg = "";
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
      if (!m.available) { backup = null; return; }
      const apps = [
        ...(m.packages || []).map((p) => ({
          name: p.package, installed: !!p.installed, aur: p.source === "aur",
        })),
        ...(m.appimages || []).map((a) => ({
          name: a.name || a.id, installed: !!a.installed, aur: false, appimage: true, id: a.id,
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
      if (r.ok) toast("Backup fetched — the previous local file is kept as a .bak.", "success");
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
      if (r.ok === false) throw r.error;
      toast("Your machine's file is synced to Drive.", "success");
    } catch (e) {
      toast(e, "error");
      syncing = false;
    }
    setTimeout(refreshStatus, 1500);
  }

  async function install(a) {
    if (a.appimage) {
      // AppImages reinstall through their normal Discover flow (release
      // resolution, arch pick) — jump there rather than half-reimplementing it
      route.set("discover");
      toast(`Search for "${a.name}" in Discover to reinstall it.`, "info", 5000);
      return;
    }
    if (a.aur) {
      // AUR builds go through the PKGBUILD review gate, same as everywhere
      aurReview.set(a.name);
      route.set("aur");
      return;
    }
    busyPkg = a.name;
    try {
      toast(`Installing ${a.name} — authentication may be required…`, "info");
      await installPackage(a.name);
      toast(`${a.name} installed`, "success");
      a.installed = true;
      backup = backup;
      refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
    busyPkg = "";
  }

  async function installAllMissing() {
    for (const a of missing.filter((x) => !x.aur)) await install(a);
    const aurLeft = missing.filter((x) => x.aur);
    if (aurLeft.length)
      toast(`${aurLeft.length} AUR package${aurLeft.length === 1 ? "" : "s"} left — each needs its PKGBUILD reviewed in the AUR view.`, "info", 6000);
  }

  $: missing = (backup?.apps || []).filter((a) => !a.installed);
  $: present = (backup?.apps || []).filter((a) => a.installed);
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
        <div class="mb-2 flex justify-end">
          <button class="btn-ghost !py-1 text-xs" on:click={installAllMissing}>Install all repo apps</button>
        </div>
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
              {#if busyPkg === a.name}
                <span class="text-xs text-zinc-400">Installing…</span>
              {:else}
                <button class="btn-primary !py-1 text-xs" on:click={() => install(a)}>
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
      </div>
    {:else if checking}
      <div class="card p-6 text-center text-sm text-zinc-400">Fetching your backup from Drive…</div>
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
