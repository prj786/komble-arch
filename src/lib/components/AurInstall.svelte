<script>
  import * as api from "../api.js";
  import { refreshPkgs } from "../actions.js";
  import { aurReview, aurQueue, droppedPkg, toast } from "../stores.js";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";
  import IconBtn from "./ui/IconBtn.svelte";
  import SearchField from "./ui/SearchField.svelte";
  import { Checkbox } from "./ui/checkbox/index.js";

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
    keys: "Importing the signing key",
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
  // the Search field binds `query`; every change schedules a search
  let lastQuery = "";
  $: if (query !== lastQuery) {
    lastQuery = query;
    onQuery();
  }
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
      toast(`Installed **${name}**`, "success");
      reviewing = null;
      pkgbuild = "";
      meta = null;
      await refreshPkgs();
      // a restore walks its AUR list: the next one's review opens by itself
      reviewNext();
    } catch (e) {
      buildError = String(e);
      showLog = true;
      toast(e, "error");
    }
    building = false;
    stage = "";
  }

  const shortKey = (k) => (k.length > 16 ? `${k.slice(0, 8)}…${k.slice(-8)}` : k);

  // The queue behind a restore ("Review AUR apps" in For you): pop the next
  // name and open its review; an empty queue just leaves the view idle.
  function reviewNext() {
    let next = "";
    aurQueue.update((q) => {
      next = q[0] || "";
      return q.slice(1);
    });
    if (next) {
      query = next;
      search();
      review(next);
    }
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
      toast("Installed the package", "success");
      localPath = "";
      await refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
    building = false;
  }
</script>

<Page title="AUR" desc="Build packages from the Arch User Repository. Komble clones the package, runs makepkg as you (never as root), then installs the result with pacman.">
  <!-- The review gate. A PKGBUILD is a shell script that runs with your
       privileges at build time and can do anything you can, so it is shown in
       full before anything executes. This is the security model, not a nicety. -->
  <Alert tone="warning" title="Read the PKGBUILD before you build">
    AUR packages are user-submitted and unreviewed. Komble shows you the PKGBUILD before it builds anything, because it runs on your machine.
  </Alert>

  <Group title="Search" well={false}>
    <SearchField bind:value={query} placeholder="Search the AUR" />
    {#if searching}
      <div class="ewe-list" aria-busy="true">
        <div class="ewe-row"><span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Searching…</span></div>
      </div>
    {:else if results.length}
      <div class="ewe-list">
        {#each results.slice(0, 40) as p (p.name)}
          <div class="ewe-row" class:is-selected={reviewing === p.name}>
            <div class="ewe-row__text">
              <div class="row-title">
                <span class="ewe-row__title">{p.name}</span>
                <span class="ewe-badge ver"><span class="ewe-badge__label">{p.version}</span></span>
              </div>
              <div class="ewe-row__desc">{p.summary}</div>
            </div>
            <div class="ewe-row__trail">
              {#if p.installed}
                <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Installed</span></span>
              {:else}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-label="Review the PKGBUILD of {p.name}" on:click={() => review(p.name)}>
                  Review PKGBUILD
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </Group>

  {#if $aurQueue.length && !building}
    <Alert tone="accent" title="From your ewe.conf: {$aurQueue.length} more AUR app{$aurQueue.length === 1 ? '' : 's'} to review after this one">
      {$aurQueue.slice(0, 4).join(", ")}{$aurQueue.length > 4 ? ", …" : ""}
      <svelte:fragment slot="actions">
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={reviewNext}>{reviewing ? "Skip to the next" : "Review the next"}</button>
        <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => aurQueue.set([])}>Stop</button>
      </svelte:fragment>
    </Alert>
  {/if}

  {#if reviewing}
    <section class="ewe-card" aria-label="PKGBUILD of {reviewing}">
      <div class="ewe-card__head">
        <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="fileCode" /></span>
        <div class="ewe-card__titles">
          <div class="ewe-card__title">PKGBUILD of {reviewing}</div>
        </div>
        <IconBtn name="x" title="Close the review" go={() => (reviewing = null)} />
      </div>
      {#if loadingPkgbuild}
        <div class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Fetching…</div>
      {:else}
        {#if meta?.validpgpkeys?.length}
          <!-- makepkg verifies signed sources against these keys and refuses
               to build without them; a fresh keyring has none, so Komble
               fetches them first (never --skippgpcheck by default). -->
          <Alert tone="info" title="The sources are PGP-signed">
            By key {#each meta.validpgpkeys as k, i}<code title={k}>{shortKey(k)}</code>{i < meta.validpgpkeys.length - 1 ? ", " : ""}{/each}.
            Komble imports {meta.validpgpkeys.length === 1 ? "it" : "them"} into your GPG keyring (<code>gpg --recv-keys</code>) before building.
            <svelte:fragment slot="actions">
              <Checkbox bind:checked={skipPgp} disabled={building} label="Skip the signature check (unsafe: only if the key can’t be fetched)" />
            </svelte:fragment>
          </Alert>
        {/if}
        <pre class="log log--tall" aria-label="PKGBUILD">{pkgbuild}</pre>
        {#if buildError}
          <!-- the toast lives eight seconds; the reason stays here until the
               card is closed or the next attempt starts -->
          <Alert tone="danger" title="Couldn’t build {reviewing}">
            <pre class="log">{showLog && buildLog ? buildLog : buildError}</pre>
            <svelte:fragment slot="actions">
              {#if buildLog}
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => (showLog = !showLog)}>
                  {showLog ? "Hide the full log" : "Show the full log"}
                </button>
              {/if}
            </svelte:fragment>
          </Alert>
        {/if}
        <div class="ewe-card__foot">
          {#if building}
            <span class="busy" role="status"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>{stage}…</span>
          {:else}
            <button class="ewe-btn ewe-btn--ghost" on:click={() => (reviewing = null)}>Cancel</button>
            <button class="ewe-btn ewe-btn--primary" on:click={build}>
              {buildError ? "Try again" : "I’ve read it, build and install"}
            </button>
          {/if}
        </div>
      {/if}
    </section>
  {/if}

  <Group title="Install a package file" desc="A local .pkg.tar.zst: something you built yourself, or dropped onto this window." well={false}>
    <div class="form-row">
      <label class="ewe-input">
        <Icon name="fileBox" />
        <input class="field-text" aria-label="Package file" placeholder="/path/to/package.pkg.tar.zst" bind:value={localPath} />
      </label>
      <button class="ewe-btn ewe-btn--secondary" on:click={pickLocal}><Icon name="folderOpen" />Browse…</button>
      <button class="ewe-btn ewe-btn--secondary" disabled={!localPath || building} on:click={installLocal}>Install</button>
    </div>
  </Group>
</Page>
