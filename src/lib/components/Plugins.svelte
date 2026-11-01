<script>
  // Plugins — what extends the shell, managed through `ewe-plugin` (the CLI
  // is the implementation; this view runs it and shows its words). One name
  // everywhere: plugins. Three groups, ONE row anatomy (PluginRow): the ones
  // ewe ships inside its own payload but does not install (opt-in since
  // 0.25 — Insomnia, the dock, Music, Cast…; one click installs one, a
  // removed one comes back the same way), the plugins from a git address or
  // a folder, and the ones another machine had. Clicking a plugin opens its
  // sheet (PluginSheet): on/off, where it comes from, what it adds and
  // needs, its options, Update / Remove.
  //
  // The trust model is the CLI's: installing never runs plugin code,
  // enabling does — unsandboxed, inside the shell. The warning says so where
  // it applies: in the Add plugin sheet, and on the sheet of every plugin
  // that is not ewe's own. A toggle restarts the shell for a second; Komble
  // is left alone.
  import { onMount, onDestroy } from "svelte";
  import { openUrl, openPath } from "@tauri-apps/plugin-opener";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { toast, pendingOptions, route } from "../stores";
  import * as api from "../api";
  import Toggle from "./ui/Toggle.svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";
  import Seg from "./ui/Seg.svelte";
  import Sheet from "./ui/Sheet.svelte";
  import { ICONS, themeIcon } from "./ui/icons.js";
  import { Checkbox } from "./ui/checkbox/index.js";
  import * as Menu from "./ui/dropdown-menu/index.js";
  import PluginRow from "./PluginRow.svelte";
  import PluginSheet from "./PluginSheet.svelte";

  const GUIDE = "https://prj786.github.io/docs/plugins/";
  const EXAMPLE = "https://github.com/prj786/ewe-plugin-example";

  let data = null;        // the last `list --json`
  let loaded = false;
  let error = "";         // why there is no list (an old desktop, mostly)
  let show = "all";       // the filter: all | on | off (not installed)
  let selected = null;    // id whose sheet is open
  let busy = "";          // id (or "add" / "restore" / "create") with a command in flight
  let confirming = null;  // id whose Remove is armed (the second press removes)
  let timer;

  // ── Add plugin… (a sheet: the git address, the warning in context)
  let adding = false;
  let url = "";
  let enableNew = true;
  // ── New plugin… (a sheet: ewe-plugin create in a folder of the user's choosing)
  let creating = false;
  let newId = "";
  let newName = "";
  let newKinds = { "bar-widget": true, "desktop-widget": false, panel: false, service: false,
                   "quick-tile": false, "quick-page": false, "bar-status": false, "dock-item": false };
  let newDir = "";
  // the kinds by their API names: this form is for whoever writes the QML
  const KIND_LABELS = {
    "bar-widget": "Bar widget", "desktop-widget": "Desktop widget", panel: "Panel", service: "Service",
    overlay: "Overlay", menu: "Menu", "quick-tile": "Quick settings tile", "quick-page": "Quick settings page",
    "bar-status": "Bar status", "dock-item": "Dock item"
  };

  // the catalog: absent on an ewe before 0.25 → the group is not shown
  $: addons = data && Array.isArray(data.available) ? data.available : [];
  $: addonIds = new Set(addons.map((a) => a.id));
  // the installed entry of a plugin (validity, options) lives in plugins[]
  $: byId = new Map(data ? data.plugins.filter((p) => p.installed).map((p) => [p.id, p]) : []);
  $: fetchableIds = new Set(data ? data.missing : []);

  // what a plugin still needs on this machine: the packages Install puts in
  // first, or, when every package is here, the commands still missing
  const needsOf = (a) => (a.missing?.packages?.length ? a.missing.packages : a.missing?.commands || []);
  const isUrl = (s) => typeof s === "string" && /^(https:\/\/|ssh:\/\/|git@)/.test(s);

  // ONE entry shape for every plugin, whatever group it is in — the row and
  // the sheet read only this
  function fromCatalog(a) {
    const live = byId.get(a.id) || null;
    const installed = !!a.installed;
    return {
      id: a.id,
      name: a.name || a.id,
      description: (live && live.description) || a.description || "",
      icon: themeIcon(a.icon || (live && live.icon)),
      category: a.category || (live && live.category) || "",
      version: (live && live.version) || a.version || "",
      kinds: (live && live.kinds?.length ? live.kinds : a.kinds) || [],
      requires: a.requires || (live && live.requires) || null,
      missing: installed ? null : a.missing || null,
      where: "ewe",
      source: "bundled",
      homepage: a.repo || (live && live.homepage) || "",
      state: installed ? "installed" : "available",
      enabled: installed && live ? !!live.enabled : !!a.enabled,
      valid: installed && live ? live.valid !== false : true,
      problems: (live && live.problems) || [],
      live: installed ? live : null,
      fetchable: false
    };
  }
  function fromInstalled(p) {
    return {
      id: p.id,
      name: p.name || p.id,
      description: p.description || "",
      icon: p.icon ? themeIcon(p.icon) : ICONS.puzzle,
      category: p.category || (p.bundled ? "" : p.git ? "" : "Hand-made"),
      version: p.version || "",
      kinds: p.kinds || [],
      requires: p.requires || null,
      missing: null,
      where: p.bundled ? "ewe" : p.linked ? "linked" : p.git ? "git" : "hand",
      source: p.source || "",
      homepage: p.homepage || "",
      state: "installed",
      enabled: !!p.enabled,
      valid: p.valid !== false,
      problems: p.problems || [],
      live: p,
      fetchable: false
    };
  }
  function fromElsewhere(p) {
    return {
      id: p.id,
      name: p.name || p.id,
      description:
        p.source === "local"
          ? "A local folder on that machine: there is nothing to fetch."
          : p.source === "bundled"
            ? "Comes with ewe and installs with the ewe package."
            : "",
      icon: ICONS.puzzle,
      category: "",
      version: "",
      kinds: [],
      requires: null,
      missing: null,
      where: "gone",
      source: p.source || "",
      homepage: "",
      state: "missing",
      enabled: !!p.enabled,
      valid: true,
      problems: [],
      live: null,
      fetchable: fetchableIds.has(p.id) && isUrl(p.source)
    };
  }

  $: fromEwe = addons.map(fromCatalog);
  $: others = data ? data.plugins.filter((p) => p.installed && !addonIds.has(p.id)).map(fromInstalled) : [];
  $: elsewhere = data ? data.plugins.filter((p) => !p.installed && !addonIds.has(p.id)).map(fromElsewhere) : [];
  $: entries = [...fromEwe, ...others, ...elsewhere];
  // the sheet's plugin, looked up again on every re-read; gone (a removed
  // plugin from a git address) → load() closes the sheet
  $: current = selected ? entries.find((x) => x.id === selected) || null : null;

  const keep = (x) => (show === "on" ? x.state === "installed" && x.enabled : show === "off" ? x.state !== "installed" : true);
  $: shownEwe = fromEwe.filter(keep);
  $: shownOthers = others.filter(keep);
  $: shownElsewhere = elsewhere.filter(keep);
  $: shownCount = shownEwe.length + shownOthers.length + shownElsewhere.length;
  const filters = [
    ["all", "All"],
    ["on", "On"],
    ["off", "Not installed"]
  ];

  async function load() {
    try {
      data = await api.pluginList();
      error = "";
      const known = (id) => data.plugins.some((p) => p.id === id) || (data.available || []).some((a) => a.id === id);
      if (selected && !known(selected)) selected = null;
    } catch (e) {
      data = null;
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  // `komble --options=<id>` (pendingOptions, ewe Settings' "Dock options",
  // the shell): open that plugin's sheet once the list is read
  function takeOptions() {
    const id = $pendingOptions;
    if (!id || !data) return;
    pendingOptions.set("");
    if (entries.some((x) => x.id === id)) selected = id;
    else toast(`${id} isn’t a plugin this machine knows. Add it with Add plugin…`, "info");
  }
  $: $pendingOptions, entries, takeOptions();

  async function run(id, fn, okMsg) {
    if (busy) return;
    busy = id;
    try {
      const out = await fn();
      toast(okMsg || (typeof out === "string" ? out.split("\n").pop() : "") || "Done", "success");
      return true;
    } catch (e) {
      toast(e, "error", 8000);
      return false;
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

  async function add() {
    const u = url.trim();
    if (!u) return;
    const ok = await run("add", () => api.pluginAdd(u, enableNew), null);
    if (ok) {
      url = "";
      adding = false;
    }
  }
  const setEnabled = (x, on) =>
    run(x.id, () => api.pluginSetEnabled(x.id, on), `${on ? "Turned on" : "Turned off"} **${x.name}**. The shell is restarting.`);
  const update = (x) => run(x.id, () => api.pluginUpdate(x.id));
  const restoreAll = () => run("restore", () => api.pluginRestore(), "Restored the plugins");

  // Install, whatever the group: a first-party plugin from the payload (what
  // it still needs on this machine — packages — is installed first, by the
  // backend, through the ordinary pacman path: hence the password note), or
  // one another machine had, cloned from its git address
  function install(x) {
    if (x.state === "missing") {
      run(x.id, () => api.pluginAdd(x.source, x.enabled), `Restored **${x.name}**`);
      return;
    }
    const pkgs = x.missing?.packages || [];
    if (pkgs.length) toast(`Installing ${pkgs.join(", ")} first. You may be asked for your password…`, "info", 15000);
    run(x.id, () => api.pluginInstall(x.id), `Installed **${x.name}**. The shell is restarting.`);
  }

  function askConfirm(id) {
    confirming = id;
    setTimeout(() => { if (confirming === id) confirming = null; }, 4000);
  }
  // two presses: the first arms Remove (in the sheet), the second removes
  function remove(x) {
    if (confirming !== x.id) return askConfirm(x.id);
    confirming = null;
    const after = x.where === "ewe" && addonIds.has(x.id)
      ? `Removed **${x.name}**. Install brings it back any time.`
      : x.where === "ewe"
        ? `Removed **${x.name}**. ewe updates leave it out; ewe-plugin seed --restore ${x.id} brings it back.`
        : `Removed **${x.name}**`;
    run(x.id, () => api.pluginRemove(x.id), after);
  }
  // ⋯ → Remove…: open the sheet with Remove armed, so it is confirmed in context
  function removeFromMenu(x) {
    selected = x.id;
    askConfirm(x.id);
  }

  function menuOf(x) {
    if (x.state !== "installed") return [];
    return [
      { label: "Options…", icon: "sliders", run: () => (selected = x.id) },
      ...(x.where === "git" ? [{ label: "Update", icon: "refresh", run: () => update(x) }] : []),
      { label: "Remove…", icon: "trash", danger: true, run: () => removeFromMenu(x) }
    ];
  }
  const needsLabel = (x) => {
    const n = x.state === "available" ? needsOf(x) : [];
    return n.length ? `Needs ${n.join(", ")}` : "";
  };
  const descOf = (x) => (x.state === "installed" && !x.valid ? x.problems[0] || "This plugin doesn’t load." : x.description || (x.where === "gone" ? x.source : x.id));

  // re-read when the window comes back (the terminal, a restore in
  // ewe Sync) and once a minute; never while a command runs
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

{#snippet row(x)}
  <PluginRow
    name={x.name}
    desc={descOf(x)}
    icon={x.icon}
    tag={x.state === "missing" ? (x.enabled ? "Was on" : "Was off") : x.category}
    needs={needsLabel(x)}
    needsTitle="Installed first, with your password"
    lit={x.state === "installed" && x.enabled && x.valid}
    state={x.state}
    enabled={x.enabled}
    broken={x.state === "installed" && !x.valid}
    busy={busy === x.id}
    selected={selected === x.id}
    canInstall={x.state === "available" || x.fetchable}
    menu={menuOf(x)}
    open={() => (selected = x.id)}
    toggle={(v) => setEnabled(x, v)}
    install={() => install(x)}
  />
{/snippet}

<Page title="Plugins" desc="Extras for your desktop, from ewe or from a git address." class={current || adding || creating ? "has-sheet" : ""}>
  <svelte:fragment slot="actions">
    <button class="ewe-btn ewe-btn--primary" disabled={!!error} on:click={() => ((adding = true), (creating = false), (selected = null))}><Icon name="plus" />Add plugin…</button>
    <Menu.Root>
      <Menu.Trigger class="ewe-iconbtn ewe-iconbtn--ghost" aria-label="More: new plugin, open the guide">
        <Icon name="more" />
      </Menu.Trigger>
      <Menu.Content align="end">
        <Menu.Item onSelect={() => ((creating = true), (adding = false), (selected = null))}>
          <Icon name="fileCode" /><span class="ewe-menu__label">New plugin…</span>
        </Menu.Item>
        <Menu.Item onSelect={() => openUrl(GUIDE)}>
          <Icon name="book" /><span class="ewe-menu__label">Open the guide</span>
        </Menu.Item>
      </Menu.Content>
    </Menu.Root>
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
      <div class="ewe-empty__actions">
        <button class="ewe-btn ewe-btn--secondary" on:click={() => route.set("updates")}>Check for updates</button>
      </div>
    </div>
  {:else}
    {#if data.safeMode}
      <Alert tone="warning" title="Safe mode: this session loaded no plugins">
        The shell restarted three times within a minute. Turned on at the time: {data.suspects.join(", ")}. Turn the culprit off and the rest come back at the next start.
      </Alert>
    {/if}

    <Seg options={filters} value={show} label="Show" size="sm" picked={(v) => (show = v)} />

    {#if shownCount === 0}
      <div class="ewe-empty ewe-empty--compact">
        <span class="ewe-empty__icon"><Icon name="puzzle" /></span>
        <div class="ewe-empty__title">{show === "on" ? "No plugin is on" : "Every plugin is installed"}</div>
        <div class="ewe-empty__desc">{show === "on" ? "Turn one on with its switch, or install one from ewe." : "Add one from a git address with Add plugin…"}</div>
        <div class="ewe-empty__actions">
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => (show = "all")}>Show all plugins</button>
        </div>
      </div>
    {/if}

    {#if shownEwe.length}
      <!-- From ewe: the catalog ewe ships, nothing installed until asked -->
      <Group title="From ewe · {shownEwe.length}">
        <div class="plugin-list" role="list" aria-label="Plugins from ewe">
          {#each shownEwe as x (x.id)}{@render row(x)}{/each}
        </div>
      </Group>
    {/if}

    {#if shownOthers.length}
      <Group title="{addons.length ? 'Other plugins' : 'Installed'} · {shownOthers.length}">
        <div class="plugin-list" role="list" aria-label={addons.length ? "Other plugins" : "Installed plugins"}>
          {#each shownOthers as x (x.id)}{@render row(x)}{/each}
        </div>
      </Group>
    {:else if show === "all"}
      <!-- nothing from a git address yet: the next step, not an empty box -->
      <Group title={addons.length ? "Other plugins" : "Installed"}>
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="git" /></span>
          <div class="ewe-empty__title">{addons.length ? "No plugins from a git address yet" : "No plugins yet"}</div>
          <div class="ewe-empty__desc">Add one by its git address, or start your own from the reference plugin.</div>
          <div class="ewe-empty__actions">
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => ((adding = true), (selected = null))}><Icon name="plus" />Add plugin…</button>
            <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => ((creating = true), (selected = null))}>New plugin…</button>
          </div>
        </div>
      </Group>
    {/if}

    {#if shownElsewhere.length}
      <Group title="From your other machine · {shownElsewhere.length}">
        <svelte:fragment slot="action">
          {#if elsewhere.filter((x) => x.fetchable).length > 1}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={restoreAll} disabled={!!busy}>
              {busy === "restore" ? "Cloning…" : `Restore all (${elsewhere.filter((x) => x.fetchable).length})`}
            </button>
          {/if}
        </svelte:fragment>
        <div class="plugin-list" role="list" aria-label="Plugins from your other machine">
          {#each shownElsewhere as x (x.id)}{@render row(x)}{/each}
        </div>
      </Group>
    {/if}
  {/if}
</Page>

{#if current}
  {#key current.id}
    <PluginSheet
      e={current}
      {busy}
      {confirming}
      {run}
      {setSetting}
      {setEnabled}
      {install}
      {update}
      {remove}
      close={() => (selected = null)}
    />
  {/key}
{/if}

<!-- Add plugin: the git address, and the trust model right where it applies -->
<Sheet open={adding} title="Add plugin" subtitle="From a git address" initialFocus="input" onClose={() => (adding = false)}>
  <span slot="lead" class="ewe-row__lead ewe-row__lead--tile sheet-tile" aria-hidden="true"><Icon name="git" /></span>
  <label class="ewe-field">
    <span class="ewe-field__label">Git address</span>
    <span class="ewe-input">
      <Icon name="git" />
      <input
        class="field-text"
        placeholder="https://github.com/someone/ewe-something.git"
        bind:value={url}
        on:keydown={(e) => e.key === "Enter" && add()}
        disabled={busy === "add"}
      />
    </span>
    <span class="ewe-field__helper">https://… or git@… — Komble clones it and checks its manifest.</span>
  </label>
  <div class="ewe-switch-row">
    <span class="switch-text">
      <span class="switch-text__title">Turn on after install</span>
      <span class="switch-text__desc">Off: it is installed and does nothing until you turn it on</span>
    </span>
    <Toggle on={enableNew} label="Turn on after install" toggled={() => (enableNew = !enableNew)} />
  </div>
  <Alert tone="warning" title="Read it before you turn it on">
    Installing never runs plugin code. Turning it on does: unsandboxed, inside your shell, with everything the desktop can do.
    <svelte:fragment slot="actions">
      <button class="ewe-link" on:click={() => openUrl(EXAMPLE)}>Open the reference plugin<Icon name="external" /></button>
    </svelte:fragment>
  </Alert>
  <svelte:fragment slot="footer">
    <button class="ewe-btn ewe-btn--ghost" on:click={() => (adding = false)}>Cancel</button>
    <button class="ewe-btn ewe-btn--primary" on:click={add} disabled={!url.trim() || busy === "add"}>
      {#if busy === "add"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Cloning…{:else}Add{/if}
    </button>
  </svelte:fragment>
</Sheet>

<!-- New plugin: a plugin repository of your own -->
<Sheet open={creating} title="New plugin" subtitle="A repository to write your own in" initialFocus="input" onClose={() => (creating = false)}>
  <span slot="lead" class="ewe-row__lead ewe-row__lead--tile sheet-tile" aria-hidden="true"><Icon name="fileCode" /></span>
  <p class="sheet-text">
    A working plugin per kind, a README that explains the contract, an MIT license and a first commit. You write the QML; ewe places it and shows its settings here.
  </p>
  <label class="ewe-field">
    <span class="ewe-field__label">ID</span>
    <span class="ewe-input"><input class="field-text" placeholder="acme.clock" bind:value={newId} /></span>
    <span class="ewe-field__helper">namespace.name</span>
  </label>
  <label class="ewe-field">
    <span class="ewe-field__label">Name <span class="ewe-field__optional">Optional</span></span>
    <span class="ewe-input"><input class="field-text" placeholder="Big clock" bind:value={newName} /></span>
  </label>
  <div class="ewe-field">
    <span class="ewe-field__label">Kinds</span>
    <div class="checks">
      {#each Object.keys(newKinds) as k}
        <Checkbox bind:checked={newKinds[k]} label={KIND_LABELS[k]} />
      {/each}
    </div>
  </div>
  <div class="ewe-field">
    <span class="ewe-field__label">Folder</span>
    <button class="ewe-btn ewe-btn--secondary folder-btn" on:click={pickDir}>
      <Icon name="folderOpen" /><span class="folder-btn__path">{newDir ? newDir : "Choose a folder…"}</span>
    </button>
  </div>
  <svelte:fragment slot="footer">
    <button class="ewe-btn ewe-btn--ghost" on:click={() => (creating = false)}>Cancel</button>
    <button class="ewe-btn ewe-btn--primary" disabled={busy === "create" || !newId.trim() || !newDir} on:click={create}>Create</button>
  </svelte:fragment>
</Sheet>
