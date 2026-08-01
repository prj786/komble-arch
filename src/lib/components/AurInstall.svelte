<script>
  import * as api from "../api.js";
  import { refreshPkgs } from "../actions.js";
  import { aurReview, droppedPkg, toast } from "../stores.js";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  let query = "";
  let results = [];
  let searching = false;

  // the package whose PKGBUILD is on screen; nothing builds until this is read
  let reviewing = null;
  let pkgbuild = "";
  let loadingPkgbuild = false;
  let building = false;
  let stage = "";

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
    loadingPkgbuild = true;
    try {
      pkgbuild = await api.aurPkgbuild(name);
    } catch (e) {
      toast(e, "error");
      reviewing = null;
    }
    loadingPkgbuild = false;
  }

  async function build() {
    if (!reviewing) return;
    building = true;
    stage = "building";
    try {
      await api.aurInstall(reviewing);
      toast(`${reviewing} installed`, "success");
      reviewing = null;
      pkgbuild = "";
      await refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
    building = false;
    stage = "";
  }

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
        <pre class="max-h-96 overflow-auto rounded-lg bg-zinc-100 p-3 text-[11px] leading-relaxed dark:bg-zinc-900">{pkgbuild}</pre>
        <div class="mt-3 flex items-center justify-end gap-2">
          {#if building}
            <span class="text-xs text-zinc-400">{stage}…</span>
          {:else}
            <button class="btn-primary !py-1 text-xs" on:click={build}>
              I have read this — build and install
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
