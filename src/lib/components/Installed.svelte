<script>
  import { installed, trackedPkgs, progress, toast } from "../stores";
  import { refreshInstalled, refreshPkgs } from "../actions";
  import * as api from "../api";
  import { formatDate } from "../utils";

  let confirming = null;

  function askConfirm(key) {
    confirming = key;
    setTimeout(() => {
      if (confirming === key) confirming = null;
    }, 4000);
  }

  async function removeAppimage(entry) {
    if (confirming !== `ai:${entry.id}`) return askConfirm(`ai:${entry.id}`);
    confirming = null;
    try {
      await api.removeAppimage(entry.id);
      toast(`${entry.name} removed`, "success");
      refreshInstalled();
    } catch (e) {
      toast(e, "error");
    }
  }

  async function removeDebPkg(d) {
    if (confirming !== `pkg:${d.package}`) return askConfirm(`pkg:${d.package}`);
    confirming = null;
    try {
      toast(`Removing ${d.package} — authentication may be required…`, "info");
      await api.removePackage(d.package);
      toast(`${d.package} removed`, "success");
      refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
  }
</script>

<div class="h-full overflow-y-auto px-6 py-6">
  <h1 class="text-xl font-bold tracking-tight">Installed</h1>
  <p class="mb-4 text-sm text-zinc-400 dark:text-zinc-500">
    Apps managed by Komble on this machine
  </p>

  <div class="section-title">AppImages · {$installed.length}</div>
  {#if $installed.length === 0}
    <div class="card p-6 text-center text-sm text-zinc-400">
      No AppImages yet — install something from Discover.
    </div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each $installed as entry (entry.id)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          {#if entry.iconUrl}
            <img src={entry.iconUrl} alt="" class="h-9 w-9 rounded-lg object-contain" on:error={(e) => (e.currentTarget.style.display = "none")} />
          {:else}
            <div class="flex h-9 w-9 items-center justify-center rounded-lg text-sm font-bold text-white" style="background: var(--accent)">
              {(entry.name || "?").slice(0, 1).toUpperCase()}
            </div>
          {/if}
          <div class="min-w-0 flex-1">
            <div class="flex items-baseline gap-2">
              <span class="truncate font-medium">{entry.name}</span>
              <span class="rounded bg-zinc-100 px-1.5 py-0.5 text-[11px] text-zinc-500 dark:bg-zinc-700/70 dark:text-zinc-300">
                {entry.version}
              </span>
            </div>
            <div class="truncate text-xs text-zinc-400 dark:text-zinc-500">
              {entry.path} · {formatDate(entry.installedAt)}
            </div>
          </div>
          {#if $progress[entry.id]}
            <span class="text-xs text-zinc-400">updating…</span>
          {:else}
            <button
              class="{confirming === `ai:${entry.id}` ? 'btn-danger' : 'btn-ghost'} !py-1 text-xs"
              on:click={() => removeAppimage(entry)}
            >
              {confirming === `ai:${entry.id}` ? "Really remove?" : "Remove"}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <div class="section-title">Packages installed via Komble · {$trackedPkgs.length}</div>
  {#if $trackedPkgs.length === 0}
    <div class="card p-6 text-center text-sm text-zinc-400">
      Packages you install through Komble will appear here.
    </div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each $trackedPkgs as d (d.package)}
        <div class="card flex items-center gap-3.5 px-4 py-3">
          <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-zinc-200 text-sm font-bold text-zinc-500 dark:bg-zinc-700 dark:text-zinc-300">
            {d.package.slice(0, 1).toUpperCase()}
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-baseline gap-2">
              <span class="truncate font-medium">{d.package}</span>
              <span class="rounded bg-zinc-100 px-1.5 py-0.5 text-[11px] text-zinc-500 dark:bg-zinc-700/70 dark:text-zinc-300">
                {d.version}
              </span>
            </div>
            <div class="truncate text-xs text-zinc-400 dark:text-zinc-500">
              {d.description || d.source} · {formatDate(d.installedAt)}
            </div>
          </div>
          <button
            class="{confirming === `pkg:${d.package}` ? 'btn-danger' : 'btn-ghost'} !py-1 text-xs"
            on:click={() => removeDebPkg(d)}
          >
            {confirming === `pkg:${d.package}` ? "Really remove?" : "Remove"}
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>
