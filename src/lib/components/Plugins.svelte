<script>
  // Plugins — third-party bar widgets, panels and services for the shell,
  // managed through `ewe-plugin` (the CLI is the implementation; this view
  // runs it and shows its words). The trust model is the CLI's: installing
  // never runs plugin code, enabling does — unsandboxed, inside the shell —
  // so the warning stays on screen, not in a modal that gets clicked away.
  // A toggle restarts the shell for a second; Komble is left alone.
  import { onMount, onDestroy } from "svelte";
  import { openUrl, openPath } from "@tauri-apps/plugin-opener";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
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
  // "New plugin…": ewe-plugin create in a folder of the user's choosing
  let creating = false;
  let newId = "";
  let newName = "";
  let newKinds = { "bar-widget": true, "desktop-widget": false, panel: false, service: false };
  let newDir = "";
  const KIND_LABELS = { "bar-widget": "Bar widget", "desktop-widget": "Desktop widget", panel: "Panel", service: "Service" };
  let expanded = null;    // id whose settings form is open
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

  async function pickDir() {
    const d = await openDialog({ directory: true, multiple: false, title: "Create the plugin in…" });
    if (d) newDir = typeof d === "string" ? d : d.path;
  }
  async function create() {
    const kinds = Object.keys(newKinds).filter((k) => newKinds[k]);
    if (!newId.trim() || !kinds.length || !newDir) return;
    await run("create", async () => {
      const dest = await api.pluginCreate(newId.trim(), newName, kinds, newDir);
      toast(`Created ${dest} — a git repo; edit the QML, then \`ewe-plugin dev\` it or push it`, "success", 9000);
      try { await openPath(dest); } catch {}
      creating = false; newId = ""; newName = "";
    });
  }
  // settings: typed by the manifest; the CLI refuses what does not fit
  let pending = {};
  function setSetting(p, key, value) {
    clearTimeout(pending[p.id + key]);
    pending[p.id + key] = setTimeout(() => run(p.id, () => api.pluginSet(p.id, key, value), `${p.name || p.id}: ${key} saved`), 350);
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
    run(p.id, () => api.pluginRemove(p.id), p.bundled ? `${p.name || p.id} removed — ewe updates will leave it out (ewe-plugin seed --restore ${p.id} brings it back)` : `${p.name || p.id} removed`);
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
    <div class="mb-3 flex justify-end">
      <button class="btn-ghost !py-1 text-xs" on:click={() => (creating = !creating)}>{creating ? "Cancel" : "New plugin…"}</button>
    </div>
    {#if creating}
      <div class="card mb-3 p-4">
        <div class="mb-2 text-sm font-medium">A new plugin repository</div>
        <p class="mb-3 text-xs text-dim">You get a working plugin per kind, a README that explains the contract, an MIT licence and a first commit. You write the QML; ewe places it, and shows its settings as a form here.</p>
        <div class="grid gap-2 sm:grid-cols-2">
          <input class="input" placeholder="id — namespace.name, e.g. acme.clock" bind:value={newId} />
          <input class="input" placeholder="Name (optional)" bind:value={newName} />
        </div>
        <div class="mt-2 flex flex-wrap gap-3 text-sm">
          {#each Object.keys(newKinds) as k}
            <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={newKinds[k]} /> {KIND_LABELS[k]}</label>
          {/each}
        </div>
        <div class="mt-2 flex items-center gap-2">
          <button class="btn-ghost !py-1 text-xs" on:click={pickDir}>{newDir ? "Folder: " + newDir : "Choose a folder…"}</button>
          <span class="flex-1"></span>
          <button class="btn-primary !py-1 text-xs" disabled={busy === "create" || !newId.trim() || !newDir} on:click={create}>Create</button>
        </div>
      </div>
    {/if}
    {#if installed.length === 0}
      <div class="card p-6 text-center text-sm text-dim">
        No plugins yet — paste a git URL above, start from the reference plugin, or make one with New plugin…
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
                {#if p.bundled}<span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim" title="Comes with ewe — removing it is remembered; a later ewe update will not bring it back">bundled</span>
                {:else if !p.git}<span class="rounded bg-elevated px-1.5 py-0.5 text-[11px] text-dim">hand-made</span>{/if}
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
              {#if (p.settingsSchema && p.settingsSchema.length) || p.widget}
                <button class="btn-ghost !py-1 text-xs" on:click={() => (expanded = expanded === p.id ? null : p.id)}>{expanded === p.id ? "Close" : "Options"}</button>
              {/if}
              <Toggle on={p.enabled} disabled={!p.valid} toggled={() => setEnabled(p, !p.enabled)} />
            {/if}
          </div>
          {#if expanded === p.id}
            <div class="card -mt-1 flex flex-col gap-3 px-4 py-3">
              {#if p.widget}
                <div class="flex flex-wrap items-center gap-3 text-sm">
                  <span class="font-medium">On the desktop</span>
                  <button class="btn-ghost !py-1 text-xs" on:click={() => run(p.id, () => api.pluginArrange(), "Arrange mode — drag on the desktop, Esc when done")}>Arrange…</button>
                  <label class="flex items-center gap-2 text-xs"><span>Sticky (above windows)</span><Toggle on={p.widget.layer === "top"} toggled={() => run(p.id, () => api.pluginPlace(p.id, p.widget.layer === "top" ? "desktop" : "top", null), "Saved")} /></label>
                  <label class="flex items-center gap-2 text-xs"><span>Shown</span><Toggle on={p.widget.visible !== false} toggled={() => run(p.id, () => api.pluginPlace(p.id, null, p.widget.visible === false), "Saved")} /></label>
                  <span class="text-xs text-dim">at {p.widget.x}, {p.widget.y}{p.widget.output ? " on " + p.widget.output : ""}</span>
                </div>
              {/if}
              {#each p.settingsSchema || [] as s (s.key)}
                <label class="flex items-center gap-3 text-sm">
                  <span class="min-w-0 flex-1 truncate">{s.label || s.key}</span>
                  {#if s.type === "bool"}
                    <Toggle on={!!p.settings[s.key]} toggled={() => setSetting(p, s.key, !p.settings[s.key])} />
                  {:else if s.type === "int"}
                    <input class="input w-24" type="number" min={s.min} max={s.max} value={p.settings[s.key]} on:change={(e) => setSetting(p, s.key, e.currentTarget.value)} />
                  {:else if s.type === "choice"}
                    <select class="input w-40" value={p.settings[s.key]} on:change={(e) => setSetting(p, s.key, e.currentTarget.value)}>
                      {#each s.choices || [] as c}<option value={c}>{c}</option>{/each}
                    </select>
                  {:else if s.type === "color"}
                    <input type="color" class="h-7 w-9 cursor-pointer rounded-md border-0 bg-transparent p-0" value={p.settings[s.key]} on:change={(e) => setSetting(p, s.key, e.currentTarget.value)} />
                  {:else}
                    <input class="input w-48" value={p.settings[s.key] ?? ""} on:change={(e) => setSetting(p, s.key, e.currentTarget.value)} />
                  {/if}
                </label>
              {/each}
            </div>
          {/if}
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
                {p.source === "local" ? "a local directory on that machine — nothing to fetch" : p.source === "bundled" ? "comes with ewe — installs with the ewe package" : p.source}
              </div>
            </div>
            {#if p.source && p.source !== "local" && p.source !== "bundled"}
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
