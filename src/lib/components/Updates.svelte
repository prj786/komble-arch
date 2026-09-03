<script>
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { updatesInfo, settings, progress, systemInfo, toast } from "../stores";
  import { refreshInstalled } from "../actions";
  import * as api from "../api";

  let checking = false;
  let working = false;
  let updatingAll = false;
  let fixingContrib = false;

  // ── the ewe desktop: the DE itself + Komble + ewe-settings, updated from
  // here so nobody ever hand-downloads a .tar.zst or .pkg.tar.zst again
  let ewe = null; // ewe_status() result
  let fp = []; // first_party_status() rows (komble-arch, ewe-settings)
  let eweWorking = false;
  let eweNeedsTerminal = false;
  let eweLog = []; // streamed update.sh events while an in-app update runs
  let unlistenEwe = null;

  // ONE busy flag for every action button: "Update all" and "Upgrade system"
  // used to be clickable during each other's quiet moments, which launched two
  // makepkg pipelines side by side (the backend now also refuses a second
  // concurrent upgrade outright).
  $: busyAny = working || eweWorking || updatingAll || checking;

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
            note: ewe.dirty ? "working tree has local changes" : ""
          }
        ]
      : []),
    ...fp.map((f) => ({
      id: f.pkg,
      name: f.pkg,
      current: f.installed || "not installed",
      latest: f.latest || "?",
      update: !!f.updateAvailable,
      note: f.error || ""
    }))
  ];
  $: desktopUpdates = desktopRows.filter((r) => r.update).length;

  onMount(async () => {
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
      toast("Installing pacman-contrib — authentication may be required…", "info");
      await api.installPacmanContrib();
      systemInfo.update((i) => (i ? { ...i, checkupdates: true } : i));
      toast("pacman-contrib installed — checking for system updates", "success");
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
    // the desktop and its two apps, checked in parallel with each other
    const [st, rows] = await Promise.all([
      api.eweStatus($settings.githubToken).catch(() => null),
      api.firstPartyStatus($settings.githubToken).catch(() => [])
    ]);
    ewe = st;
    fp = rows;
    res.desktop =
      (st && st.updateAvailable ? 1 : 0) + rows.filter((r) => r.updateAvailable).length;
    updatesInfo.set(res);
    checking = false;
    // nudge the bar to re-probe now, so its count moves with this one
    api.pokeShellUpdates().catch(() => {});
  }
  onMount(check);

  async function updateOne(u) {
    try {
      await api.updateAppimage(u.id, $settings.githubToken);
      toast(`${u.name} updated to ${u.latest}`, "success");
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
  async function systemUpgradeAll() {
    working = true;
    const hasRepo = $updatesInfo.packages.some((p) => p.source === "repo");
    const hasAur = $updatesInfo.packages.some((p) => p.source === "aur");
    try {
      if (hasRepo) {
        toast("Upgrading system packages — authentication may be required…", "info");
        await api.systemUpgrade();
      }
      if (hasAur) {
        toast("Rebuilding AUR packages (clone → build → install)…", "info");
        await api.aurUpgrade();
      }
      toast("System upgraded", "success");
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
      if (desktopUpdates > 0) await updateDesktop();
      if ($updatesInfo.packages.length) await systemUpgradeAll();
    } finally {
      updatingAll = false;
    }
  }

  // ── one button for the whole desktop: the two apps first (pkexec pacman -U
  // from their GitHub releases), then the DE itself via its update.sh contract.
  async function updateDesktop() {
    eweWorking = true;
    eweNeedsTerminal = false;
    eweLog = [];
    try {
      for (const f of fp.filter((x) => x.updateAvailable)) {
        toast(`Updating ${f.pkg} ${f.installed || ""} → ${f.latest}…`, "info");
        await api.installFirstParty(f.pkg, $settings.githubToken);
        toast(`${f.pkg} updated to ${f.latest}`, "success");
      }
      if (ewe && ewe.updateAvailable) {
        toast("Updating the ewe desktop…", "info");
        await api.eweUpdate();
        toast("ewe desktop updated — the shell restarts itself", "success");
      }
    } catch (e) {
      if (String(e) === "needs-terminal") {
        eweNeedsTerminal = true;
        toast("The desktop update needs a terminal for sudo — use the button below.", "info", 7000);
      } else {
        toast(e, "error");
      }
    }
    await check();
    eweWorking = false;
  }

  async function updateInTerminal() {
    try {
      await api.eweUpdateTerminal();
      toast("Continuing in the terminal — this list refreshes on the next check.", "info", 6000);
    } catch (e) {
      toast(e, "error");
    }
  }

  async function refreshLists() {
    working = true;
    try {
      await api.refreshLists();
      toast("Package lists refreshed", "success");
      await check();
    } catch (e) {
      toast(e, "error");
    }
    working = false;
  }

  $: total = $updatesInfo.appimages.length + $updatesInfo.packages.length + desktopUpdates;
</script>

<div class="h-full overflow-y-auto px-4 py-5 sm:px-6 sm:py-6">
  <div class="mb-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
    <div class="min-w-0">
      <h1 class="text-xl font-bold tracking-tight">Updates</h1>
      <p class="text-sm text-dim dark:text-dim">
        {#if checking}Checking…{:else if total === 0}Everything is up to date ✓{:else}{total} update{total === 1 ? "" : "s"} available{/if}
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <button class="btn-ghost" disabled={busyAny} on:click={refreshLists} title="Re-read the package databases (checkupdates — never pacman -Sy)">
        Refresh lists
      </button>
      <button class="btn-ghost" disabled={busyAny} on:click={check}>Check again</button>
      {#if total > 0}
        <button class="btn-primary" disabled={busyAny} on:click={updateAll}>Update all</button>
      {/if}
    </div>
  </div>

  <!-- ── the ewe desktop: DE + Komble + ewe-settings, one Update button ── -->
  {#if desktopRows.length}
    <div class="section-title">ewe Desktop · {desktopUpdates > 0 ? `${desktopUpdates} update${desktopUpdates === 1 ? "" : "s"}` : "up to date"}</div>
    <div class="mb-2 flex flex-col gap-2">
      {#each desktopRows as r (r.id)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium">{r.name}</div>
            <div class="truncate text-xs text-dim">
              {#if r.update}
                {r.current} <span class="mx-1">→</span>
                <span class="font-medium text-dim ">{r.latest}</span>
              {:else}
                {r.current}
              {/if}
              {#if r.note}<span class="ml-2 text-warning dark:text-warning">{r.note}</span>{/if}
            </div>
          </div>
          {#if $progress[r.id]}
            <span class="text-xs text-dim">{$progress[r.id].stage || ""}…</span>
          {:else if r.update}
            <span class="rounded px-2 py-0.5 text-[11px] uppercase tracking-wide text-dim">update</span>
          {:else}
            <span class="text-xs font-medium text-success dark:text-success">✓</span>
          {/if}
        </div>
      {/each}
      {#if desktopUpdates > 0}
        <div class="flex items-center justify-end gap-2">
          {#if eweNeedsTerminal}
            <button class="btn-primary !py-1 whitespace-nowrap text-xs" on:click={updateInTerminal}>
              Update in a terminal…
            </button>
          {/if}
          <button class="btn-primary !py-1 whitespace-nowrap text-xs" disabled={busyAny} on:click={updateDesktop}>
            {eweWorking ? "Updating…" : "Update desktop"}
          </button>
        </div>
      {/if}
      {#if eweLog.length}
        <pre class="card max-h-48 overflow-y-auto whitespace-pre-wrap p-3 text-[11px] leading-snug text-dim">{eweLog.join("\n")}</pre>
      {/if}
    </div>
  {/if}

  <div class="section-title">AppImages · {$updatesInfo.appimages.length}</div>
  {#if $updatesInfo.appimages.length === 0}
    <div class="card p-5 text-center text-sm text-dim">
      {checking ? "Checking…" : "All AppImages are current."}
    </div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each $updatesInfo.appimages as u (u.id)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium">{u.name}</div>
            <div class="text-xs text-dim">
              {u.current} <span class="mx-1">→</span>
              <span class="font-medium text-dim ">{u.latest}</span>
            </div>
          </div>
          {#if $progress[u.id]}
            <span class="text-xs tabular-nums text-dim">
              {$progress[u.id].phase === "integrating"
                ? "Integrating…"
                : $progress[u.id].total > 0
                  ? `${Math.round(($progress[u.id].downloaded / $progress[u.id].total) * 100)}%`
                  : "…"}
            </span>
          {:else}
            <button class="btn-primary !py-1 text-xs" on:click={() => updateOne(u)}>Update</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <div class="section-title">System packages · {$updatesInfo.packages.length}</div>
  {#if contribMissing}
    <div class="card mb-2 flex items-center gap-3 border-[var(--warning)] px-4 py-3">
      <div class="min-w-0 flex-1 text-sm">
        <span class="font-medium">Full system updates need pacman-contrib.</span>
        <span class="text-dim">
          It provides checkupdates, the only safe way to list pending repo updates — without it
          only AUR and AppImage updates appear here.</span>
      </div>
      <button class="btn-primary !py-1 whitespace-nowrap text-xs" disabled={fixingContrib} on:click={installContrib}>
        {fixingContrib ? "Installing…" : "Install it"}
      </button>
    </div>
  {/if}
  {#if $updatesInfo.packages.length === 0}
    <div class="card p-5 text-center text-sm text-dim">
      {checking ? "Checking…" : contribMissing ? "Repo updates unknown — install pacman-contrib above." : "Everything is up to date."}
    </div>
  {:else}
    <div class="mb-2 flex items-center justify-between gap-3">
      <p class="text-xs text-dim">
        Arch upgrades as a whole. Updating individual packages against a newer
        database is a partial upgrade and is unsupported.
      </p>
      <button class="btn-primary !py-1 whitespace-nowrap text-xs" disabled={busyAny} on:click={systemUpgradeAll}>
        Upgrade system
      </button>
    </div>
    <div class="flex flex-col gap-2">
      {#each $updatesInfo.packages as p (p.name)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium">{p.name}</div>
            <div class="truncate text-xs text-dim">
              {p.current} <span class="mx-1">→</span>
              <span class="font-medium text-dim ">{p.latest}</span>
            </div>
          </div>
          {#if $progress[p.name]}
            <span class="text-xs text-dim">{$progress[p.name].stage || ""}…</span>
          {:else}
            <span class="rounded px-2 py-0.5 text-[11px] uppercase tracking-wide text-dim">
              {p.source}
            </span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if $updatesInfo.errors.length}
    <div class="section-title">Warnings</div>
    <div class="card space-y-1 border-[var(--warning)] p-4 text-xs text-warning dark:text-warning">
      {#each $updatesInfo.errors as err}
        <div>{err}</div>
      {/each}
    </div>
  {/if}
</div>
