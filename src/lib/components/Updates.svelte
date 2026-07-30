<script>
  import { onMount } from "svelte";
  import { updatesInfo, settings, progress, toast } from "../stores";
  import { refreshInstalled } from "../actions";
  import * as api from "../api";

  let checking = false;
  let working = false;

  async function check() {
    checking = true;
    const res = { appimages: [], packages: [], errors: [], self: null, checkedAt: Date.now() };
    try {
      res.self = await api.checkSelfUpdate();
    } catch {
      res.self = null; // updater not configured in this build
    }
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
    updatesInfo.set(res);
    checking = false;
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

  // There is deliberately no per-package upgrade. Upgrading one package against
  // a newer sync database is a PARTIAL UPGRADE, which is unsupported on Arch and
  // the most common way to break an install. The only offer is the whole system.
  async function systemUpgradeAll() {
    working = true;
    try {
      await api.systemUpgrade();
      toast("System packages upgraded", "success");
      updatesInfo.update((i) => ({ ...i, packages: [] }));
    } catch (e) {
      toast(e, "error");
    }
    working = false;
  }

  async function updateAll() {
    for (const u of [...$updatesInfo.appimages]) await updateOne(u);
    if ($updatesInfo.packages.length) await systemUpgradeAll();
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

  $: total = $updatesInfo.appimages.length + $updatesInfo.packages.length;
</script>

<div class="h-full overflow-y-auto px-6 py-6">
  <div class="mb-4 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-bold tracking-tight">Updates</h1>
      <p class="text-sm text-zinc-400 dark:text-zinc-500">
        {#if checking}Checking…{:else if total === 0}Everything is up to date ✓{:else}{total} update{total === 1 ? "" : "s"} available{/if}
      </p>
    </div>
    <div class="flex gap-2">
      <button class="btn-ghost" disabled={working || checking} on:click={refreshLists} title="pkexec apt-get update">
        Refresh lists
      </button>
      <button class="btn-ghost" disabled={checking} on:click={check}>Check again</button>
      {#if total > 0}
        <button class="btn-primary" disabled={working || checking} on:click={updateAll}>Update all</button>
      {/if}
    </div>
  </div>

  {#if $updatesInfo.self}
    <div class="section-title">Komble</div>
    <div class="card flex items-center gap-3 px-4 py-3">
      <div class="min-w-0 flex-1">
        <span class="font-medium">Komble {$updatesInfo.self}</span>
        <span class="text-sm text-zinc-400"> is available</span>
      </div>
      <span class="text-xs text-zinc-400">Update via your package manager or a new .deb build</span>
    </div>
  {/if}

  <div class="section-title">AppImages · {$updatesInfo.appimages.length}</div>
  {#if $updatesInfo.appimages.length === 0}
    <div class="card p-5 text-center text-sm text-zinc-400">
      {checking ? "Checking…" : "All AppImages are current."}
    </div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each $updatesInfo.appimages as u (u.id)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium">{u.name}</div>
            <div class="text-xs text-zinc-400">
              {u.current} <span class="mx-1">→</span>
              <span class="font-medium text-zinc-500 dark:text-zinc-300">{u.latest}</span>
            </div>
          </div>
          {#if $progress[u.id]}
            <span class="text-xs tabular-nums text-zinc-400">
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
  {#if $updatesInfo.packages.length === 0}
    <div class="card p-5 text-center text-sm text-zinc-400">
      {checking ? "Checking…" : "Everything is up to date."}
    </div>
  {:else}
    <div class="mb-2 flex items-center justify-between gap-3">
      <p class="text-xs text-zinc-400">
        Arch upgrades as a whole. Updating individual packages against a newer
        database is a partial upgrade and is unsupported.
      </p>
      <button class="btn-primary !py-1 whitespace-nowrap text-xs" disabled={working} on:click={systemUpgradeAll}>
        Upgrade system
      </button>
    </div>
    <div class="flex flex-col gap-2">
      {#each $updatesInfo.packages as p (p.name)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium">{p.name}</div>
            <div class="truncate text-xs text-zinc-400">
              {p.current} <span class="mx-1">→</span>
              <span class="font-medium text-zinc-500 dark:text-zinc-300">{p.latest}</span>
            </div>
          </div>
          <span class="rounded px-2 py-0.5 text-[11px] uppercase tracking-wide text-zinc-400">
            {p.source}
          </span>
        </div>
      {/each}
    </div>
  {/if}

  {#if $updatesInfo.errors.length}
    <div class="section-title">Warnings</div>
    <div class="card space-y-1 border-amber-400/40 p-4 text-xs text-amber-700 dark:text-amber-400">
      {#each $updatesInfo.errors as err}
        <div>{err}</div>
      {/each}
    </div>
  {/if}
</div>
