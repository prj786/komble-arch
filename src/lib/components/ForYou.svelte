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
  let backup = null; // backup_packages() result
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

  async function readBackup() {
    try {
      backup = await api.backupPackages();
    } catch {
      backup = null;
    }
  }

  onMount(async () => {
    await refreshStatus();
    await readBackup();
    if (google?.signedIn) checkBackup();
    timer = setInterval(() => {
      if (syncing || google?.busy) refreshStatus();
    }, 2000);
  });
  onDestroy(() => clearInterval(timer));

  /** Ask the shell to fetch the cloud bundle, then poll the cache file. */
  async function checkBackup() {
    checking = true;
    try {
      const r = (await api.qsIpc("fetchPackages")).trim();
      if (r === "not-signed-in") {
        checking = false;
        return;
      }
      const before = backup?.fetchedAt;
      for (let i = 0; i < 20; i++) {
        await new Promise((res) => setTimeout(res, 700));
        await readBackup();
        if (backup && backup.fetchedAt !== before) break;
      }
    } catch (e) {
      toast(e, "error");
    }
    checking = false;
  }

  async function syncNow() {
    syncing = true;
    try {
      await api.qsIpc("syncNow");
      toast("Syncing your apps and settings to Drive…", "info");
    } catch (e) {
      toast(e, "error");
      syncing = false;
    }
    setTimeout(refreshStatus, 1500);
  }

  async function install(a) {
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
        <button class="btn-ghost" disabled={checking} on:click={checkBackup}>
          {checking ? "Checking…" : "Check backup"}
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
    <div class="card flex flex-col items-center gap-3 p-8 text-center">
      <div class="text-3xl">☁️</div>
      <p class="max-w-md text-sm text-zinc-500 dark:text-zinc-400">
        Sign in with Google to back up the list of apps you install and get them back on any
        ewe machine.
      </p>
      <button class="btn-primary" disabled={google.busy === "signin"} on:click={() => api.qsIpc("signIn").then(refreshStatus)}>
        {google.busy === "signin" ? "Waiting for the browser…" : "Sign in with Google"}
      </button>
      {#if google.error}<p class="text-xs text-amber-500">{google.error}</p>{/if}
    </div>
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

    {#if backup?.ok && backup.apps?.length}
      <div class="section-title">
        Apps missing from this machine · {missing.length}
        {#if backup.device}<span class="ml-2 font-normal normal-case tracking-normal">backup from {backup.device}, {fmt(backup.updatedAt)}</span>{/if}
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
