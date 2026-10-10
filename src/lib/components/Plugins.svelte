<script>
  // Plugins — what extends the shell, managed through `ewe-plugin` (the CLI
  // is the implementation; this view runs it and shows its words). One name
  // everywhere: plugins. Two halves: the ones ewe ships inside its own
  // payload but does not install (opt-in since 0.25 — Insomnia, the dock,
  // Music, Cast…; one click installs one, a removed one comes back the same
  // way), and the plugins from a git URL. The trust model is the CLI's:
  // installing never runs plugin code, enabling does — unsandboxed, inside
  // the shell — so the warning stays on screen, not in a modal that gets
  // clicked away. A toggle restarts the shell for a second; Komble is left
  // alone. Options open in a dialog (PluginOptions).
  import { onMount, onDestroy } from "svelte";
  import { openUrl, openPath } from "@tauri-apps/plugin-opener";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { toast, pendingOptions } from "../stores";
  import * as api from "../api";
  import Toggle from "./ui/Toggle.svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";
  import { themeIcon } from "./ui/icons.js";
  import { Checkbox } from "./ui/checkbox/index.js";
  import PluginOptions from "./PluginOptions.svelte";

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
  let newKinds = { "bar-widget": true, "desktop-widget": false, panel: false, service: false,
                   "quick-tile": false, "quick-page": false, "bar-status": false, "dock-item": false };
  let newDir = "";
  const KIND_LABELS = {
    "bar-widget": "Bar widget", "desktop-widget": "Desktop widget", panel: "Panel", service: "Service",
    overlay: "Overlay", menu: "Menu", "quick-tile": "Quick settings tile", "quick-page": "Quick settings page",
    "bar-status": "Bar status", "dock-item": "Dock item"
  };
  let expanded = null;    // id whose Options dialog is open
  let busy = "";          // id (or "add" / "restore") with a command in flight
  let confirming = null;
  let timer;

  // the catalog: absent on an ewe before 0.25 → the group is not shown
  $: addons = data && Array.isArray(data.available) ? data.available : [];
  $: addonIds = new Set(addons.map((a) => a.id));
  // the installed entry of a first-party plugin (validity, options) lives in plugins[]
  $: byId = new Map(data ? data.plugins.map((p) => [p.id, p]) : []);
  // everything else: plugins from a URL or a folder, never the catalog's
  $: installed = data ? data.plugins.filter((p) => p.installed && !addonIds.has(p.id)) : [];
  $: missing = data ? data.plugins.filter((p) => !p.installed && !addonIds.has(p.id)) : [];
  $: fetchable = data ? data.missing.filter((id) => !addonIds.has(id)) : [];
  // the Options dialog's plugin, looked up again on every re-read so it shows
  // what the CLI last answered; gone (removed) → the dialog closes
  $: optionsOf = expanded ? byId.get(expanded) || null : null;

  async function load() {
    try {
      data = await api.pluginList();
      error = "";
      // a plugin removed under an open Options dialog closes it, and never
      // reopens it on a later reinstall
      if (expanded && !data.plugins.some((p) => p.id === expanded && p.installed)) expanded = null;
      takeOptions();
    } catch (e) {
      data = null;
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  // `komble --options=<id>` (pendingOptions): open that plugin's Options
  // once the list says it is installed; otherwise just land on the page
  function takeOptions() {
    const id = $pendingOptions;
    if (!id || !data) return;
    pendingOptions.set("");
    if (data.plugins.some((p) => p.id === id && p.installed)) expanded = id;
    else toast(`${id} is not installed. Install it below, then open its Options.`, "info");
  }
  $: $pendingOptions, takeOptions();

  async function run(id, fn, okMsg) {
    if (busy) return;
    busy = id;
    try {
      const out = await fn();
      toast(okMsg || (typeof out === "string" ? out.split("\n").pop() : "") || "Done", "success");
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
      toast(`Created **${dest}**, a git repository. Edit the QML, then try it with ewe-plugin dev or push it.`, "success", 9000);
      try { await openPath(dest); } catch {}
      creating = false; newId = ""; newName = "";
    });
  }
  // settings: typed by the manifest; the CLI refuses what does not fit
  let pending = {};
  // one write after another, never gated on `busy`: routed through run(), a
  // second change inside the 350 ms window was dropped while the first write
  // was still in flight, and the re-read then showed the old value (2026-09-20)
  let settingChain = Promise.resolve();
  let settingReload;
  function setSetting(p, key, value) {
    clearTimeout(pending[p.id + key]);
    pending[p.id + key] = setTimeout(() => {
      settingChain = settingChain
        .then(async () => {
          try {
            await api.pluginSet(p.id, key, value);
            const label = (p.settingsSchema || []).find((o) => o.key === key)?.label || key;
            toast(`Saved **${label}** for ${p.name || p.id}`, "success");
          } catch (e) {
            toast(e, "error", 8000);
          }
        })
        .then(() => {
          clearTimeout(settingReload);
          settingReload = setTimeout(load, 1200);
        });
    }, 350);
  }

  function add() {
    const u = url.trim();
    if (!u) return;
    run("add", () => api.pluginAdd(u, enableNew), null).then(() => (url = ""));
  }
  const setEnabled = (p, on) => run(p.id, () => api.pluginSetEnabled(p.id, on), `${on ? "Turned on" : "Turned off"} **${p.name || p.id}**. The shell is restarting.`);
  const update = (p) => run(p.id, () => api.pluginUpdate(p.id));
  const install = (p) => run(p.id, () => api.pluginAdd(p.source, p.enabled), `Restored **${p.id}**`);
  const restoreAll = () => run("restore", () => api.pluginRestore(), "Restored the plugins");

  // a first-party plugin: from the payload, so there is nothing to fetch; what it still
  // needs on this machine (packages) is installed first, by the backend,
  // through the ordinary pacman path — hence the password note
  const needs = (a) => [...(a.missing?.packages || []), ...(a.missing?.commands || [])];
  function installAddon(a) {
    const pkgs = a.missing?.packages || [];
    if (pkgs.length) toast(`Installing ${pkgs.join(", ")} first. You may be asked for your password…`, "info", 15000);
    run(a.id, () => api.pluginInstall(a.id), `Installed **${a.name || a.id}**. The shell is restarting.`);
  }
  // Options: declared settings, a Show in bar switch, or a desktop widget
  const hasOptions = (p) =>
    !!p && ((p.settingsSchema && p.settingsSchema.length) || p.widget || (p.bar && p.bar.toggle !== false));

  function askConfirm(key) {
    confirming = key;
    setTimeout(() => { if (confirming === key) confirming = null; }, 4000);
  }
  function remove(p) {
    if (confirming !== p.id) return askConfirm(p.id);
    confirming = null;
    const name = p.name || p.id;
    const after = addonIds.has(p.id)
      ? `Removed **${name}**. Install brings it back any time.`
      : p.bundled
        ? `Removed **${name}**. ewe updates leave it out; ewe-plugin seed --restore ${p.id} brings it back.`
        : `Removed **${name}**`;
    if (expanded === p.id) expanded = null;
    run(p.id, () => api.pluginRemove(p.id), after);
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

<Page title="Plugins" desc="What comes with ewe, and plugins from a git URL: the dock, bar widgets, panels and services for the shell">
  <svelte:fragment slot="actions">
    <button class="ewe-btn ewe-btn--ghost" on:click={() => openUrl(GUIDE)}><Icon name="book" />Open the guide</button>
  </svelte:fragment>

  {#if !loaded}
    <div class="ewe-empty" aria-busy="true">
      <span class="ewe-empty__icon"><span class="ewe-spinner ewe-spinner--xl" aria-hidden="true"></span></span>
      <div class="ewe-empty__title">Reading your plugins…</div>
    </div>
  {:else if error}
    <div class="ewe-empty" role="alert">
      <span class="ewe-empty__icon"><Icon name="puzzle" /></span>
      <div class="ewe-empty__title">Plugins need ewe 0.14 or newer</div>
      <div class="ewe-empty__desc">{error}</div>
    </div>
  {:else}
    {#if data.safeMode}
      <Alert tone="warning" title="Safe mode: this session loaded no plugins">
        The shell restarted three times within a minute. Turned on at the time: {data.suspects.join(", ")}. Turn the culprit off and the rest come back at the next start.
      </Alert>
    {/if}

    {#if addons.length}
      <!-- From ewe: the catalog ewe ships, nothing installed until asked.
           One card per plugin, its state as the action: Install, or the
           on/off switch and Remove once it is here. -->
      <Group title="From ewe" desc="Part of ewe, installed only when you want them. Each takes a second and restarts the shell." well={false}>
        <div class="grid-static addons" role="list" aria-label="Plugins from ewe">
          {#each addons as a (a.id)}
            {@const live = byId.get(a.id)}
            {@const broken = a.installed && live && live.valid === false}
            {@const need = needs(a)}
            <div class="ewe-card ewe-card--compact addon" class:is-dim={broken} role="listitem" aria-label={a.name || a.id}>
              <div class="ewe-card__head">
                <span class="ewe-card__icon" class:ewe-card__icon--accent={a.installed && a.enabled && !broken}>
                  <Icon code={themeIcon(a.icon)} />
                </span>
                <div class="ewe-card__titles">
                  <div class="row-title">
                    <span class="ewe-card__title">{a.name || a.id}</span>
                    {#if a.version}<span class="ewe-badge ver"><span class="ewe-badge__label">{a.version}</span></span>{/if}
                  </div>
                  <div class="ewe-card__desc" class:text-danger={broken}>
                    {#if broken}{live.problems?.[0] || "This plugin does not load."}{:else}{a.description || a.id}{/if}
                  </div>
                </div>
              </div>
              <div class="ewe-card__foot addon__foot">
                <span class="badges">
                  {#if a.category}<span class="ewe-badge"><span class="ewe-badge__label">{a.category}</span></span>{/if}
                  {#if a.installed}
                    <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Installed</span></span>
                  {:else if need.length}
                    <span class="ewe-badge ewe-badge--warning" title="Installed first, with your password: {need.join(', ')}"><span class="ewe-badge__label">Needs {need.join(", ")}</span></span>
                  {/if}
                </span>
                <span class="addon__actions">
                  {#if busy === a.id}
                    <span class="busy" role="status"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Working…</span>
                  {:else if a.installed}
                    <button
                      class="ewe-btn ewe-btn--sm {confirming === a.id ? 'ewe-btn--danger' : 'ewe-btn--ghost'}"
                      on:click={() => remove(live || a)}
                    >
                      {confirming === a.id ? `Remove ${a.name || a.id}` : "Remove"}
                    </button>
                    {#if hasOptions(live)}
                      <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-haspopup="dialog" on:click={() => (expanded = a.id)}>
                        Options…
                      </button>
                    {/if}
                    <Toggle on={a.enabled} disabled={broken} label="{a.name || a.id} on" toggled={() => setEnabled(live || a, !a.enabled)} />
                  {:else}
                    <button class="ewe-btn ewe-btn--sm ewe-btn--primary" aria-label="Install {a.name || a.id}" on:click={() => installAddon(a)}>Install</button>
                  {/if}
                </span>
              </div>
            </div>
          {/each}
        </div>
      </Group>
    {/if}

    <Group title="Add a plugin" well={false}>
      <div class="form-row">
        <label class="ewe-input">
          <Icon name="git" />
          <input
            class="field-text"
            aria-label="Git URL of the plugin"
            placeholder="https://github.com/someone/ewe-something.git"
            bind:value={url}
            on:keydown={(e) => e.key === "Enter" && add()}
            disabled={busy === "add"}
          />
        </label>
        <label class="ewe-check">
          <Toggle on={enableNew} label="Turn on after install" toggled={() => (enableNew = !enableNew)} />
          <span class="ewe-check__label">Turn on after install</span>
        </label>
        <button class="ewe-btn ewe-btn--primary" on:click={add} disabled={!url.trim() || busy === "add"}>
          {#if busy === "add"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Cloning…{:else}Add{/if}
        </button>
      </div>
      <Alert tone="warning">
        Installing never runs plugin code. Turning it on does: unsandboxed, inside your shell, with everything the desktop can do. Read it before you turn it on.
        <svelte:fragment slot="actions">
          <button class="ewe-link" on:click={() => openUrl(EXAMPLE)}>Open the reference plugin<Icon name="external" /></button>
        </svelte:fragment>
      </Alert>
    </Group>

    <Group title="{addons.length ? 'Other plugins' : 'Installed'} · {installed.length}">
      <svelte:fragment slot="action">
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-expanded={creating} on:click={() => (creating = !creating)}>
          {#if creating}Cancel{:else}<Icon name="plus" />New plugin…{/if}
        </button>
      </svelte:fragment>
      {#if installed.length === 0}
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="puzzle" /></span>
          <div class="ewe-empty__title">{addons.length ? "No other plugins yet" : "No plugins yet"}</div>
          <div class="ewe-empty__desc">Paste a git URL above, start from the reference plugin, or make one with New plugin.</div>
        </div>
      {:else}
        {#each installed as p (p.id)}
          <div class="ewe-row ewe-row--tall" class:is-dim={!p.valid}>
            <span class="ewe-row__lead ewe-row__lead--tile" class:is-on={p.enabled && p.valid}>
              {(p.name || p.id).slice(0, 1).toUpperCase()}
            </span>
            <div class="ewe-row__text">
              <div class="row-title">
                <span class="ewe-row__title">{p.name || p.id}</span>
                {#if p.version}<span class="ewe-badge ver"><span class="ewe-badge__label">{p.version}</span></span>{/if}
                {#each p.kinds as k}<span class="ewe-badge"><span class="ewe-badge__label">{KIND_LABELS[k] || k}</span></span>{/each}
                {#if p.bundled}<span class="ewe-badge ewe-badge--accent" title="Comes with ewe. Removing it is remembered; a later ewe update won’t bring it back."><span class="ewe-badge__label">Bundled</span></span>
                {:else if !p.git}<span class="ewe-badge"><span class="ewe-badge__label">Hand-made</span></span>{/if}
              </div>
              <div class="ewe-row__desc" class:text-danger={!p.valid}>
                {#if p.valid}{p.description || p.id}{:else}{p.problems[0]}{/if}
              </div>
            </div>
            <div class="ewe-row__trail">
              {#if busy === p.id}
                <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Working…</span>
              {:else}
                {#if p.git}
                  <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" aria-label="Update {p.name || p.id}" on:click={() => update(p)}>Update</button>
                {/if}
                <button
                  class="ewe-btn ewe-btn--sm {confirming === p.id ? 'ewe-btn--danger' : 'ewe-btn--ghost'}"
                  on:click={() => remove(p)}
                >
                  {confirming === p.id ? `Remove ${p.name || p.id}` : "Remove"}
                </button>
                {#if hasOptions(p)}
                  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-haspopup="dialog" on:click={() => (expanded = p.id)}>
                    Options…
                  </button>
                {/if}
                <Toggle on={p.enabled} disabled={!p.valid} label="{p.name || p.id} on" toggled={() => setEnabled(p, !p.enabled)} />
              {/if}
            </div>
          </div>
        {/each}
      {/if}
      <svelte:fragment slot="after">
        {#if creating}
          <section class="ewe-card" aria-label="New plugin">
            <div class="ewe-card__head">
              <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="puzzle" /></span>
              <div class="ewe-card__titles">
                <div class="ewe-card__title">A new plugin repository</div>
                <div class="ewe-card__desc">
                  A working plugin per kind, a README that explains the contract, an MIT license and a first commit. You write the QML; ewe places it and shows its settings here as a form.
                </div>
              </div>
            </div>
            <div class="form-grid">
              <label class="ewe-field">
                <span class="ewe-field__label">ID</span>
                <span class="ewe-input"><input class="field-text" placeholder="acme.clock" bind:value={newId} /></span>
                <span class="ewe-field__helper">namespace.name</span>
              </label>
              <label class="ewe-field">
                <span class="ewe-field__label">Name <span class="ewe-field__optional">Optional</span></span>
                <span class="ewe-input"><input class="field-text" placeholder="Big clock" bind:value={newName} /></span>
              </label>
            </div>
            <div class="ewe-field">
              <span class="ewe-field__label">Kinds</span>
              <div class="checks">
                {#each Object.keys(newKinds) as k}
                  <Checkbox bind:checked={newKinds[k]} label={KIND_LABELS[k]} />
                {/each}
              </div>
            </div>
            <div class="ewe-card__foot">
              <button class="ewe-btn ewe-btn--secondary mr-auto" on:click={pickDir}>
                <Icon name="folderOpen" />{newDir ? newDir : "Choose a folder…"}
              </button>
              <button class="ewe-btn ewe-btn--ghost" on:click={() => (creating = false)}>Cancel</button>
              <button class="ewe-btn ewe-btn--primary" disabled={busy === "create" || !newId.trim() || !newDir} on:click={create}>Create</button>
            </div>
          </section>
        {/if}
      </svelte:fragment>
    </Group>

    {#if missing.length > 0}
      <Group title="From your other machine · {missing.length}">
        <svelte:fragment slot="action">
          {#if fetchable.length > 1}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={restoreAll} disabled={busy === "restore"}>
              {busy === "restore" ? "Cloning…" : `Restore all (${fetchable.length})`}
            </button>
          {/if}
        </svelte:fragment>
        {#each missing as p (p.id)}
          <div class="ewe-row">
            <span class="ewe-row__lead ewe-row__lead--tile">{p.id.slice(0, 1).toUpperCase()}</span>
            <div class="ewe-row__text">
              <div class="row-title">
                <span class="ewe-row__title">{p.id}</span>
                <span class="ewe-badge"><span class="ewe-badge__label">{p.enabled ? "Was on" : "Was off"}</span></span>
              </div>
              <div class="ewe-row__desc">
                {p.source === "local" ? "A local folder on that machine: nothing to fetch" : p.source === "bundled" ? "Comes with ewe and installs with the ewe package" : p.source}
              </div>
            </div>
            {#if p.source && p.source !== "local" && p.source !== "bundled"}
              <div class="ewe-row__trail">
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-label="Install {p.id}" on:click={() => install(p)} disabled={busy === p.id}>
                  {busy === p.id ? "Cloning…" : "Install"}
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </Group>
    {/if}
  {/if}
</Page>

{#if optionsOf}
  <PluginOptions p={optionsOf} {run} {setSetting} close={() => (expanded = null)} />
{/if}
