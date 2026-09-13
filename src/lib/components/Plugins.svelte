<script>
  // Plugins — third-party bar widgets, panels and services for the shell,
  // managed through `ewe-plugin` (the CLI is the implementation; this view
  // runs it and shows its words). The trust model is the CLI's: installing
  // never runs plugin code, enabling does — unsandboxed, inside the shell —
  // so the warning stays on screen, not in a modal that gets clicked away.
  // A toggle restarts the shell for a second; Komble is left alone.
  import { onMount, onDestroy } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { toast } from "../stores";
  import * as api from "../api";
  import Toggle from "./ui/Toggle.svelte";

  const GUIDE = "https://prj786.github.io/docs/plugins/";
  const EXAMPLE = "https://github.com/prj786/ewe-plugin-example";

  let data = null;        // the last `list --json`
  let loaded = false;
  let error = "";         // why there is no list (an old desktop, mostly)
  let url = "";
  let enableNew = true;
  let busy = "";          // id (or "add" / "restore") with a command in flight
  let confirming = null;
  let timer;

  $: installed = data ? data.plugins.filter((p) => p.installed) : [];
  $: missing = data ? data.plugins.filter((p) => !p.installed) : [];
  $: fetchable = data ? data.missing : [];

  async function load() {
    try {
      data = await api.pluginList();
      error = "";
    } catch (e) {
      data = null;
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  async function run(id, fn, okMsg) {
    if (busy) return;
    busy = id;
    try {
      const out = await fn();
      toast(okMsg || (out || "").split("\n").pop() || "Done", "success");
    } catch (e) {
      toast(e, "error", 8000);
    } finally {
      busy = "";
      // the shell restart takes a second; a re-read right after is honest
      setTimeout(load, 1200);
    }
  }

  function add() {
    const u = url.trim();
    if (!u) return;
    run("add", () => api.pluginAdd(u, enableNew), null).then(() => (url = ""));
  }
  const setEnabled = (p, on) => run(p.id, () => api.pluginSetEnabled(p.id, on), `${p.name || p.id} ${on ? "enabled" : "disabled"} — shell restarting`);
  const update = (p) => run(p.id, () => api.pluginUpdate(p.id));
  const install = (p) => run(p.id, () => api.pluginAdd(p.source, p.enabled), `${p.id} restored`);
  const restoreAll = () => run("restore", () => api.pluginRestore(), "Plugins restored");

  function askConfirm(key) {
    confirming = key;
    setTimeout(() => { if (confirming === key) confirming = null; }, 4000);
  }
  function remove(p) {
    if (confirming !== p.id) return askConfirm(p.id);
    confirming = null;
    run(p.id, () => api.pluginRemove(p.id), `${p.name || p.id} removed`);
  }

  // re-read when the window comes back (the terminal, a restore in
  // Settings) and once a minute; never while a command runs
  const onFocus = () => { if (!busy) load(); };
  onMount(async () => {
    await load();
    window.addEventListener("focus", onFocus);
    timer = setInterval(onFocus, 60_000);
  });
  onDestroy(() => {
    clearInterval(timer);
    window.removeEventListener("focus", onFocus);
  });
</script>

<div class="h-full overflow-y-auto px-4 py-5 sm:px-6 sm:py-6">
  <h1 class="text-xl font-bold tracking-tight">Plugins</h1>
  <p class="mb-4 text-sm text-dim dark:text-dim">
    Bar widgets, panels and services for the shell — from a git URL, into <code>~/.config/ewe/plugins</code>.
    <button class="underline decoration-dotted underline-offset-2" on:click={() => openUrl(GUIDE)}>The guide ↗</button>
  </p>

  {#if !loaded}
    <div class="card p-6 text-center text-sm text-dim">Reading…</div>
  {:else if error}
    <div class="card p-6 text-sm">
      <div class="font-medium">Plugins need ewe 0.14 or newer.</div>
      <div class="mt-1 text-dim">{error}</div>
    </div>
  {:else}
    {#if data.safeMode}
      <div class="card mb-4 border-l-4 px-4 py-3 text-sm" style="border-left-color: var(--warning)">
        <div class="font-medium">Safe mode — this session loaded no plugins.</div>
        <div class="text-dim">
          The shell restarted three times within a minute. Enabled at the time:
          {data.suspects.join(", ")}. Switch the culprit off and it will come back on the next start.
        </div>
      </div>
    {/if}

    <div class="card px-4 py-3">
      <div class="flex flex-wrap items-center gap-2">
        <input
          class="input min-w-0 flex-1"
          placeholder="https://github.com/someone/ewe-something.git"
          bind:value={url}
          on:keydown={(e) => e.key === "Enter" && add()}
          disabled={busy === "add"}
        />
        <label class="flex items-center gap-2 text-xs text-dim">
          <Toggle on={enableNew} toggled={() => (enableNew = !enableNew)} /> Enable after install
        </label>
        <button class="btn-primary !py-1.5 text-sm" on:click={add} disabled={!url.trim() || busy === "add"}>
          {busy === "add" ? "Cloning…" : "Add"}
        </button>
      </div>
      <p class="mt-2 text-xs text-dim">
        Installing never runs plugin code. Enabling does — unsandboxed, inside your shell, with
        everything the desktop can do. Read it before you switch it on.
        <button class="underline decoration-dotted underline-offset-2" on:click={() => openUrl(EXAMPLE)}>The reference plugin ↗</button>
      </p>
    </div>

    <div class="section-title">Installed · {installed.length}</div>
    {#if installed.length === 0}
      <div class="card p-6 text-center text-sm text-dim">
        No plugins yet — paste a git URL above, or start from the reference plugin.
      </div>
    {:else}
      <div class="flex flex-col gap-2">
        {#each installed as p (p.id)}
          <div class="card flex items-center gap-3.5 px-4 py-3 {p.valid ? '' : 'opacity-75'}">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-sm font-bold"
                 style={p.enabled && p.valid ? "background: var(--brand-bg); color: var(--fg-on-brand)" : "background: var(--bg-3); color: var(--fg-3)"}>
              {(p.name || p.id).slice(0, 1).toUpperCase()}
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-baseline gap-2">
                <span class="truncate font-medium">{p.name || p.id}</span>
                {#if p.version}<span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim">{p.version}</span>{/if}
                {#each p.kinds as k}<span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim">{k}</span>{/each}
                {#if !p.git}<span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim">hand-made</span>{/if}
              </div>
              <div class="truncate text-xs text-dim dark:text-dim">
                {#if p.valid}{p.description || p.id}{:else}{p.problems[0]}{/if}
              </div>
            </div>
            {#if busy === p.id}
              <span class="text-xs text-dim">working…</span>
            {:else}
              {#if p.git}
                <button class="btn-ghost !py-1 text-xs" on:click={() => update(p)}>Update</button>
              {/if}
              <button
                class="{confirming === p.id ? 'btn-danger' : 'btn-ghost'} !py-1 text-xs"
                on:click={() => remove(p)}
              >
                {confirming === p.id ? "Really remove?" : "Remove"}
              </button>
              <Toggle on={p.enabled} disabled={!p.valid} toggled={() => setEnabled(p, !p.enabled)} />
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if missing.length > 0}
      <div class="section-title flex items-center justify-between">
        <span>From your other machine · {missing.length}</span>
        {#if fetchable.length > 1}
          <button class="btn-ghost !py-1 text-xs" on:click={restoreAll} disabled={busy === "restore"}>
            {busy === "restore" ? "Cloning…" : `Restore all (${fetchable.length})`}
          </button>
        {/if}
      </div>
      <div class="flex flex-col gap-2">
        {#each missing as p (p.id)}
          <div class="card flex items-center gap-3.5 px-4 py-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-elevated text-sm font-bold text-dim">
              {p.id.slice(0, 1).toUpperCase()}
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-baseline gap-2">
                <span class="truncate font-medium">{p.id}</span>
                <span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim">{p.enabled ? "was on" : "was off"}</span>
              </div>
              <div class="truncate text-xs text-dim dark:text-dim">
                {p.source === "local" ? "a local directory on that machine — nothing to fetch" : p.source}
              </div>
            </div>
            {#if p.source && p.source !== "local"}
              <button class="btn-ghost !py-1 text-xs" on:click={() => install(p)} disabled={busy === p.id}>
                {busy === p.id ? "Cloning…" : "Install"}
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
