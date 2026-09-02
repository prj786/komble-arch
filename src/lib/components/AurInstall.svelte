<script>
  import * as api from "../api.js";
  import { refreshPkgs } from "../actions.js";
  import { aurReview, droppedPkg, toast } from "../stores.js";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  let query = "";
  let results = [];
  let searching = false;

  // the package whose PKGBUILD is on screen; nothing builds until this is read
  let reviewing = null;
  let pkgbuild = "";
  let meta = null; // .SRCINFO facts: validpgpkeys, deps — null until fetched
  let loadingPkgbuild = false;
  let building = false;
  let stage = "";
  // explicit, per-install, off by default: makepkg --skippgpcheck
  let skipPgp = false;
  // the build's own words, kept on screen after the toast is gone
  let buildLog = "";
  let buildError = "";
  let showLog = false;

  const STAGES = {
    clone: "Cloning",
    keys: "Importing signing key",
    build: "Building",
    install: "Installing",
  };

  let unlisteners = [];
  onMount(async () => {
    unlisteners.push(
      await listen("install-progress", (e) => {
        const p = e.payload;
        if (p.id === reviewing && p.stage) stage = STAGES[p.stage] || p.stage;
      })
    );
    unlisteners.push(
      await listen("install-log", (e) => {
        const p = e.payload;
        if (p.id === reviewing) buildLog = p.log || "";
      })
    );
  });
  onDestroy(() => unlisteners.forEach((u) => u()));

  let localPath = "";

  $: if ($droppedPkg) {
    localPath = $droppedPkg;
    droppedPkg.set("");
  }

  // Handed over from Discover ("Review…" on an AUR result): pre-fill the
  // search and open the PKGBUILD review for that package straight away.
  $: if ($aurReview) {
    const name = $aurReview;
    aurReview.set("");
    query = name;
    search();
    review(name);
  }

  let timer;
  function onQuery() {
    clearTimeout(timer);
    timer = setTimeout(search, 300);
  }

  async function search() {
    const q = query.trim();
    if (q.length < 2) {
      results = [];
      return;
    }
    searching = true;
    try {
      results = await api.aurSearch(q);
    } catch (e) {
      toast(e, "error");
    }
    searching = false;
  }

  async function review(name) {
    reviewing = name;
    pkgbuild = "";
    meta = null;
    skipPgp = false;
    buildLog = "";
    buildError = "";
    showLog = false;
    loadingPkgbuild = true;
    // the .SRCINFO half is informational (signing keys, deps) — its absence
    // must never block the review itself
    const metaReq = api.aurSrcinfo(name).catch(() => null);
    try {
      pkgbuild = await api.aurPkgbuild(name);
    } catch (e) {
      toast(e, "error");
      reviewing = null;
    }
    loadingPkgbuild = false;
    const m = await metaReq;
    if (reviewing === name) meta = m;
  }

  async function build() {
    if (!reviewing) return;
    const name = reviewing;
    building = true;
    stage = "Starting";
    buildLog = "";
    buildError = "";
    showLog = false;
    try {
      await api.aurInstall(name, skipPgp);
      toast(`${name} installed`, "success");
      reviewing = null;
      pkgbuild = "";
      meta = null;
      await refreshPkgs();
    } catch (e) {
      buildError = String(e);
      showLog = true;
      toast(e, "error");
    }
    building = false;
    stage = "";
  }

  const shortKey = (k) => (k.length > 16 ? `${k.slice(0, 8)}…${k.slice(-8)}` : k);

  async function pickLocal() {
    const sel = await openDialog({
      multiple: false,
      filters: [{ name: "Pacman package", extensions: ["zst", "xz", "gz"] }]
    });
    if (typeof sel === "string") localPath = sel;
  }

  async function installLocal() {
    if (!localPath) return;
    building = true;
    try {
      await api.installPackageFile(localPath);
      toast("Package installed", "success");
      localPath = "";
      await refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
    building = false;
  }
</script>

<div class="mx-auto h-full w-full max-w-4xl space-y-6 overflow-y-auto p-4 sm:p-6">
  <div>
    <h1 class="text-xl font-semibold">AUR</h1>
    <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
      Build packages from the Arch User Repository. Komble clones the package,
      runs <code>makepkg</code> as you (never as root), then installs the result
      with pacman.
    </p>
  </div>

  <!-- The review gate. A PKGBUILD is a shell script that runs with your
       privileges at build time and can do anything you can, so it is shown in
       full before anything executes. This is the security model, not a nicety. -->
  <div class="card border-amber-400/40 p-4 text-xs text-amber-700 dark:text-amber-400">
    AUR packages are user-submitted and unreviewed. Komble shows you the
    PKGBUILD before it builds anything — read it. It executes on your machine.
  </div>

  <div class="card p-4">
    <input
      class="input w-full"
      placeholder="Search the AUR…"
      bind:value={query}
      on:input={onQuery}
    />
    {#if searching}
      <p class="mt-3 text-xs text-zinc-400">Searching…</p>
    {:else if results.length}
      <div class="mt-3 flex flex-col gap-2">
        {#each results.slice(0, 40) as p (p.name)}
          <div class="flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-zinc-100 dark:hover:bg-zinc-800">
            <div class="min-w-0 flex-1">
              <div class="truncate font-medium">
                {p.name}
                <span class="ml-1 text-xs font-normal text-zinc-400">{p.version}</span>
              </div>
              <div class="truncate text-xs text-zinc-400">{p.summary}</div>
            </div>
            {#if p.installed}
              <span class="text-xs text-zinc-400">Installed</span>
            {:else}
              <button class="btn-ghost !py-1 text-xs" on:click={() => review(p.name)}>
                Review PKGBUILD
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if reviewing}
    <div class="card p-4">
      <div class="mb-2 flex items-center justify-between">
        <div class="font-medium">PKGBUILD — {reviewing}</div>
        <button class="btn-ghost !py-1 text-xs" on:click={() => (reviewing = null)}>Close</button>
      </div>
      {#if loadingPkgbuild}
        <p class="text-xs text-zinc-400">Fetching…</p>
      {:else}
        {#if meta?.validpgpkeys?.length}
          <!-- makepkg verifies signed sources against these keys and refuses
               to build without them; a fresh keyring has none, so Komble
               fetches them first (never --skippgpcheck by default). -->
          <div class="mb-2 rounded-lg border border-sky-400/40 bg-sky-500/5 px-3 py-2 text-xs text-sky-700 dark:text-sky-300">
            Sources are PGP-signed by key
            {#each meta.validpgpkeys as k, i}
              <code class="font-mono" title={k}>{shortKey(k)}</code>{i < meta.validpgpkeys.length - 1 ? ", " : ""}
            {/each}.
            Komble will import {meta.validpgpkeys.length === 1 ? "it" : "them"} into your GPG keyring
            (<code>gpg --recv-keys</code>) before building.
            <label class="mt-1.5 flex items-center gap-1.5 text-[11px] text-zinc-500 dark:text-zinc-400">
              <input type="checkbox" bind:checked={skipPgp} disabled={building} />
              Skip signature check (unsafe — only if the key cannot be fetched)
            </label>
          </div>
        {/if}
        <pre class="max-h-96 overflow-auto rounded-lg bg-zinc-100 p-3 text-[11px] leading-relaxed dark:bg-zinc-900">{pkgbuild}</pre>
        {#if buildError}
          <!-- the toast lives eight seconds; the reason stays here until the
               card is closed or the next attempt starts -->
          <div class="mt-3 rounded-lg border border-red-400/40 bg-red-500/5 p-3 text-xs text-red-700 dark:text-red-300">
            <div class="flex items-center justify-between gap-2">
              <span class="font-medium">Build failed</span>
              {#if buildLog}
                <button class="btn-ghost !py-0.5 text-[11px]" on:click={() => (showLog = !showLog)}>
                  {showLog ? "Hide full log" : "Show full log"}
                </button>
              {/if}
            </div>
            <pre class="mt-2 max-h-64 overflow-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed">{showLog && buildLog ? buildLog : buildError}</pre>
          </div>
        {/if}
        <div class="mt-3 flex items-center justify-end gap-2">
          {#if building}
            <span class="text-xs text-zinc-400">{stage}…</span>
          {:else}
            <button class="btn-primary !py-1 text-xs" on:click={build}>
              {buildError ? "Try again" : "I have read this — build and install"}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <div class="card p-4">
    <div class="mb-2 font-medium">Install a package file</div>
    <p class="mb-3 text-xs text-zinc-500 dark:text-zinc-400">
      A local <code>*.pkg.tar.zst</code> — something you built yourself, or dropped
      onto this window.
    </p>
    <div class="flex items-center gap-2">
      <input class="input flex-1" placeholder="/path/to/package.pkg.tar.zst" bind:value={localPath} />
      <button class="btn-ghost !py-1 text-xs" on:click={pickLocal}>Browse…</button>
      <button class="btn-primary !py-1 text-xs" disabled={!localPath || building} on:click={installLocal}>
        Install
      </button>
    </div>
  </div>
</div>
