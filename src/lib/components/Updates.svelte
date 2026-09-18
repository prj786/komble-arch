<script>
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { updatesInfo, settings, progress, systemInfo, toast, conflictPrompt, restartNeed } from "../stores";
  import { refreshInstalled } from "../actions";
  import * as api from "../api";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";

  let checking = false;
  let working = false;
  let updatingAll = false;
  let fixingContrib = false;

  // ── the ewe desktop: the DE itself + Komble + ewe-settings + ewe-sync,
  // here so nobody ever hand-downloads a .tar.zst or .pkg.tar.zst again
  let ewe = null; // ewe_status() result
  let fp = []; // first_party_status() rows — whatever DISCOVER lists
  let eweWorking = false;
  let eweNeedsTerminal = false;
  let eweLog = []; // streamed update.sh events while an in-app update runs
  let unlistenEwe = null;

  // ONE busy flag for every action button: "Update all" and "Upgrade system"
  // used to be clickable during each other's quiet moments, which launched two
  // makepkg pipelines side by side (the backend now also refuses a second
  // concurrent upgrade outright).
  $: busyAny = working || eweWorking || updatingAll || checking;

  // viaRepo: the [ewe] pacman repo delivers this one, so its update IS the
  // system upgrade (Arch never upgrades a single package) — the row still
  // lives up here, but it is taken out of the system list below so the same
  // package is not counted twice, and never shown green up here while it
  // waits down there.
  $: desktopRows = [
    ...(ewe && ewe.installed
      ? [
          {
            id: "ewe",
            name: "ewe desktop",
            current: ewe.version,
            latest: ewe.git
              ? ewe.updateAvailable
                ? `${ewe.behind} commit(s) behind`
                : ewe.version
              : ewe.latest || ewe.version,
            update: !!ewe.updateAvailable,
            viaRepo: !!ewe.packaged,
            note: ewe.dirty ? "The working tree has local changes" : ""
          }
        ]
      : []),
    ...fp.map((f) => ({
      id: f.pkg,
      name: f.pkg,
      current: f.installed || "not installed",
      latest: f.latest || "?",
      update: !!f.updateAvailable,
      viaRepo: f.managed === "repo",
      note: f.error || ""
    }))
  ];
  $: desktopUpdates = desktopRows.filter((r) => r.update).length;
  $: desktopViaRepo = desktopRows.some((r) => r.update && r.viaRepo);
  $: desktopOnlyViaRepo = desktopViaRepo && desktopRows.every((r) => !r.update || r.viaRepo);
  // system packages minus the ones the desktop section already shows
  $: desktopNames = new Set(desktopRows.filter((r) => r.viaRepo).map((r) => r.id));
  $: systemPkgs = $updatesInfo.packages.filter((p) => !desktopNames.has(p.name));

  const restartTitles = {
    reboot: "Restart to finish updating",
    logout: "Sign out and back in to finish updating",
    shell: "Restart the shell to finish updating",
    komble: "Relaunch Komble to run the new version"
  };
  const restartButtons = { reboot: "Restart now", logout: "Sign out", shell: "Restart shell", komble: "Relaunch Komble" };
  async function doRestart() {
    try {
      await api.restartAction($restartNeed.level);
      if ($restartNeed.level === "shell") restartNeed.set(null);
    } catch (e) {
      toast(e, "error");
    }
  }

  onMount(async () => {
    // a kernel updated by anyone (a terminal pacman, the ISO) still wants a restart
    api.restartState().then((r) => r && r.level !== "none" && !$restartNeed && restartNeed.set(r)).catch(() => {});
    unlistenEwe = await listen("ewe-update", (e) => {
      const v = e.payload || {};
      const line =
        v.event === "log"
          ? v.line
          : v.event === "phase"
            ? `${v.name}: ${v.status}${v.message ? ` — ${v.message}` : ""}`
            : v.event === "error"
              ? `error: ${v.message || ""}`
              : "";
      if (line) eweLog = [...eweLog.slice(-400), line];
    });
  });
  onDestroy(() => unlistenEwe && unlistenEwe());

  // Without checkupdates (pacman-contrib) there is NO safe way to list repo
  // updates, and the system section sits silently empty — surface that here
  // with a one-click fix instead of pretending the system is up to date.
  $: contribMissing = $systemInfo && !$systemInfo.checkupdates;

  async function installContrib() {
    fixingContrib = true;
    try {
      toast("Installing **pacman-contrib**. You may be asked for your password…", "info");
      await api.installPacmanContrib();
      systemInfo.update((i) => (i ? { ...i, checkupdates: true } : i));
      toast("Installed **pacman-contrib**. Checking for system updates…", "info");
      await check();
    } catch (e) {
      toast(e, "error");
    }
    fixingContrib = false;
  }

  async function check() {
    checking = true;
    const res = { appimages: [], packages: [], errors: [], self: null, desktop: 0, checkedAt: Date.now() };
    try {
      const r = await api.checkAppimageUpdates($settings.githubToken);
      res.appimages = r.updates;
      res.errors = r.errors;
    } catch (e) {
      res.errors = [String(e)];
    }
    try {
      res.packages = await api.listUpgradable();
    } catch (e) {
      res.errors.push(String(e));
    }
    // the desktop and its apps, checked in parallel with each other — handed
    // the repo updates above, so a package the [ewe] repo carries reads the
    // same here as in the system list
    const repoUpdates = res.packages
      .filter((p) => p.source === "repo")
      .map((p) => ({ name: p.name, latest: p.latest }));
    const [st, rows] = await Promise.all([
      api.eweStatus($settings.githubToken, repoUpdates).catch(() => null),
      api.firstPartyStatus($settings.githubToken, repoUpdates).catch(() => [])
    ]);
    ewe = st;
    fp = rows;
    // count only what the system list does not already count (a repo-
    // delivered row is one of res.packages already)
    res.desktop =
      (st && st.updateAvailable && !st.packaged ? 1 : 0) +
      rows.filter((r) => r.updateAvailable && r.managed !== "repo").length;
    updatesInfo.set(res);
    checking = false;
    // nudge the bar to re-probe now, so its count moves with this one
    api.pokeShellUpdates().catch(() => {});
  }
  onMount(check);

  async function updateOne(u) {
    try {
      await api.updateAppimage(u.id, $settings.githubToken);
      toast(`Updated **${u.name}** to ${u.latest}`, "success");
      refreshInstalled();
      updatesInfo.update((i) => ({ ...i, appimages: i.appimages.filter((x) => x.id !== u.id) }));
    } catch (e) {
      toast(e, "error");
    }
  }

  // There is deliberately no per-REPO-package upgrade. Upgrading one package
  // against a newer sync database is a PARTIAL UPGRADE, which is unsupported on
  // Arch and the most common way to break an install. Repo packages go up as a
  // whole (-Syu) — and since -Syu never touches foreign packages, the AUR ones
  // are then rebuilt one by one, or they would sit in this list forever.
  // pacman -Syu with its two questions answered by a person: a conflict it
  // would have asked about becomes a dialog (and, on consent, a rerun that
  // says yes to exactly that), and anything that is not live until a
  // restart becomes the restart dialog.
  async function runSystemUpgrade(acceptRemovals = false) {
    const r = acceptRemovals ? await api.systemUpgradeAcceptRemovals() : await api.systemUpgrade();
    if (!r.ok) {
      if (r.conflicts && r.conflicts.length) {
        const yes = await new Promise((resolve) => conflictPrompt.set({ conflicts: r.conflicts, resolve }));
        if (yes) return runSystemUpgrade(true);
        throw "Update canceled. Nothing was changed.";
      }
      throw r.error || "The update failed.";
    }
    if (r.restart && r.restart.level !== "none") restartNeed.set(r.restart);
    return r;
  }

  async function systemUpgradeAll() {
    working = true;
    const hasRepo = $updatesInfo.packages.some((p) => p.source === "repo");
    const hasAur = $updatesInfo.packages.some((p) => p.source === "aur");
    try {
      if (hasRepo) {
        toast("Updating system packages. You may be asked for your password…", "info");
        await runSystemUpgrade();
      }
      if (hasAur) {
        toast("Rebuilding AUR packages: clone, build, install…", "info");
        await api.aurUpgrade();
      }
      toast("System updated", "success");
    } catch (e) {
      toast(e, "error");
    }
    // never trust optimism — re-count from disk so nothing stale survives
    await check();
    working = false;
  }

  async function updateAll() {
    updatingAll = true;
    try {
      for (const u of [...$updatesInfo.appimages]) await updateOne(u);
      // updateDesktop re-checks when done, so a system upgrade it ran for a
      // repo-delivered row is not run a second time here
      if (desktopUpdates > 0) await updateDesktop();
      if ($updatesInfo.packages.length) await systemUpgradeAll();
    } finally {
      updatingAll = false;
    }
  }

  // ── one button for the whole desktop: release-delivered apps first (pkexec
  // pacman -U from their GitHub releases), then a git/tarball DE via its
  // update.sh contract — and whatever the [ewe] repo delivers goes with the
  // system upgrade, which is the only way a repo package moves on Arch.
  async function updateDesktop() {
    eweWorking = true;
    eweNeedsTerminal = false;
    eweLog = [];
    let viaRepo = false;
    try {
      for (const f of fp.filter((x) => x.updateAvailable && x.managed !== "repo")) {
        toast(`Updating **${f.pkg}** to ${f.latest}…`, "info");
        await api.installFirstParty(f.pkg, $settings.githubToken);
        toast(`Updated **${f.pkg}** to ${f.latest}`, "success");
      }
      if (ewe && ewe.updateAvailable && !ewe.packaged) {
        toast("Updating the ewe desktop…", "info");
        await api.eweUpdate();
        toast("Updated the ewe desktop. The shell restarts by itself.", "success");
      }
      viaRepo = desktopViaRepo;
    } catch (e) {
      if (String(e) === "needs-terminal") {
        eweNeedsTerminal = true;
        toast("The desktop update needs a terminal for sudo. Use “Update in a terminal”.", "warning", 7000);
      } else {
        toast(e, "error");
      }
    }
    eweWorking = false;
    if (viaRepo) await systemUpgradeAll(); // re-checks on its own
    else await check();
  }

  async function updateInTerminal() {
    try {
      await api.eweUpdateTerminal();
      toast("Continuing in the terminal. This list refreshes on the next check.", "info", 6000);
    } catch (e) {
      toast(e, "error");
    }
  }

  async function refreshLists() {
    working = true;
    try {
      await api.refreshLists();
      toast("Refreshed the package lists", "success");
      await check();
    } catch (e) {
      toast(e, "error");
    }
    working = false;
  }

  $: total = $updatesInfo.appimages.length + systemPkgs.length + desktopUpdates;
</script>

<Page title="Updates" desc={checking ? "Checking…" : total === 0 ? "Everything is up to date" : `${total} update${total === 1 ? "" : "s"} available`}>
  <svelte:fragment slot="actions">
    <button class="ewe-btn ewe-btn--ghost" disabled={busyAny} on:click={refreshLists} title="Read the package databases again (checkupdates, never pacman -Sy)">
      Refresh lists
    </button>
    <button class="ewe-btn ewe-btn--secondary" disabled={busyAny} aria-busy={checking} on:click={check}>
      {#if checking}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--neutral" aria-hidden="true"></span>Checking…{:else}<Icon name="refresh" />Check again{/if}
    </button>
    {#if total > 0}
      <button class="ewe-btn ewe-btn--primary" disabled={busyAny} aria-busy={updatingAll} on:click={updateAll}>
        {#if updatingAll}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Updating…{:else}<Icon name="download" />Update all{/if}
      </button>
    {/if}
  </svelte:fragment>

  {#if $restartNeed && $restartNeed.level !== "none"}
    <!-- kept until acted on: the restart dialog's "Later" leaves this -->
    <Alert tone="accent" title={restartTitles[$restartNeed.level]}>
      {$restartNeed.reasons.join(" · ")}
      <svelte:fragment slot="actions">
        <button class="ewe-btn ewe-btn--sm ewe-btn--primary" on:click={doRestart}>{restartButtons[$restartNeed.level]}</button>
      </svelte:fragment>
    </Alert>
  {/if}

  <!-- ── the ewe desktop: DE + every first-party app, one Update button ── -->
  {#if desktopRows.length}
    <Group title="ewe desktop · {desktopUpdates > 0 ? `${desktopUpdates} update${desktopUpdates === 1 ? '' : 's'}` : 'up to date'}">
      {#each desktopRows as r (r.id)}
        <div class="ewe-row">
          <div class="ewe-row__text">
            <div class="ewe-row__title">{r.name}</div>
            <div class="ewe-row__desc ver">
              {#if r.update}{r.current}<span class="row-arrow">→</span>{r.latest}{:else}{r.current}{/if}{#if r.note}<span class="text-warning"> · {r.note}</span>{/if}
            </div>
          </div>
          <div class="ewe-row__trail">
            {#if $progress[r.id]}
              <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>{$progress[r.id].stage || "Working"}…</span>
            {:else if r.update}
              <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">Update</span></span>
            {:else}
              <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Up to date</span></span>
            {/if}
          </div>
        </div>
      {/each}
      <svelte:fragment slot="after">
        {#if desktopUpdates > 0}
          <div class="group-foot">
            {#if desktopViaRepo}
              <p class="note">Delivered by the [ewe] repository, so it updates with the system.</p>
            {/if}
            {#if eweNeedsTerminal}
              <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={updateInTerminal}>
                <Icon name="terminal" />Update in a terminal…
              </button>
            {/if}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={busyAny} on:click={updateDesktop}>
              {eweWorking || (working && desktopViaRepo) ? "Updating…" : desktopOnlyViaRepo ? "Update system" : "Update desktop"}
            </button>
          </div>
        {/if}
        {#if eweLog.length}
          <pre class="log" aria-label="Update log">{eweLog.join("\n")}</pre>
        {/if}
      </svelte:fragment>
    </Group>
  {/if}

  <Group title="AppImages · {$updatesInfo.appimages.length}">
    {#if $updatesInfo.appimages.length === 0}
      <div class="ewe-empty ewe-empty--compact">
        {#if checking}
          <span class="ewe-empty__icon"><span class="ewe-spinner" aria-hidden="true"></span></span>
          <div class="ewe-empty__title">Checking…</div>
        {:else}
          <span class="ewe-empty__icon"><Icon name="success" /></span>
          <div class="ewe-empty__title">All AppImages are up to date</div>
        {/if}
      </div>
    {:else}
      {#each $updatesInfo.appimages as u (u.id)}
        {@const p = $progress[u.id]}
        <div class="ewe-row">
          <div class="ewe-row__text">
            <div class="ewe-row__title">{u.name}</div>
            <div class="ewe-row__desc ver">{u.current}<span class="row-arrow">→</span>{u.latest}</div>
          </div>
          <div class="ewe-row__trail">
            {#if p}
              <span class="busy">
                <span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>
                {p.phase === "integrating" ? "Adding to the app menu…" : p.total > 0 ? `Updating… ${Math.round((p.downloaded / p.total) * 100)}%` : "Updating…"}
              </span>
            {:else}
              <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-label="Update {u.name}" on:click={() => updateOne(u)}>
                <Icon name="download" />Update
              </button>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </Group>

  <Group title="System packages · {systemPkgs.length}">
    <svelte:fragment slot="action">
      {#if systemPkgs.length}
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={busyAny} on:click={systemUpgradeAll}>
          {working ? "Updating…" : "Update system"}
        </button>
      {/if}
    </svelte:fragment>
    {#if systemPkgs.length === 0}
      <div class="ewe-empty ewe-empty--compact">
        {#if checking}
          <span class="ewe-empty__icon"><span class="ewe-spinner" aria-hidden="true"></span></span>
          <div class="ewe-empty__title">Checking…</div>
        {:else if contribMissing}
          <span class="ewe-empty__icon"><Icon name="warning" /></span>
          <div class="ewe-empty__title">Repository updates unknown</div>
          <div class="ewe-empty__desc">Install pacman-contrib below to see them.</div>
        {:else}
          <span class="ewe-empty__icon"><Icon name="success" /></span>
          <div class="ewe-empty__title">Everything is up to date</div>
        {/if}
      </div>
    {:else}
      {#each systemPkgs as p (p.name)}
        <div class="ewe-row ewe-row--dense">
          <div class="ewe-row__text">
            <div class="ewe-row__title">{p.name}</div>
            <div class="ewe-row__desc ver">{p.current}<span class="row-arrow">→</span>{p.latest}</div>
          </div>
          <div class="ewe-row__trail">
            {#if $progress[p.name]}
              <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>{$progress[p.name].stage || "Working"}…</span>
            {:else}
              <span class="ewe-badge {p.source === 'aur' ? 'ewe-badge--warning' : ''}"><span class="ewe-badge__label">{p.source === "aur" ? "AUR" : p.source}</span></span>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
    <svelte:fragment slot="after">
      {#if contribMissing}
        <Alert tone="warning" title="Full system updates need pacman-contrib">
          It provides checkupdates, the only safe way to list pending repository updates. Without it, only AUR and AppImage updates appear here.
          <svelte:fragment slot="actions">
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={fixingContrib} on:click={installContrib}>
              {fixingContrib ? "Installing…" : "Install pacman-contrib"}
            </button>
          </svelte:fragment>
        </Alert>
      {/if}
      {#if systemPkgs.length}
        <p class="note">Arch updates as a whole. Updating single packages against a newer database is a partial update, which Arch doesn’t support.</p>
      {/if}
    </svelte:fragment>
  </Group>

  {#if $updatesInfo.errors.length}
    <Group title="Warnings" well={false}>
      <Alert tone="warning" title="Some checks didn’t finish">
        {#each $updatesInfo.errors as err}<div>{err}</div>{/each}
      </Alert>
    </Group>
  {/if}
</Page>
