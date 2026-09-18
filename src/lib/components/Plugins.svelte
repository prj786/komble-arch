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
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";
  import { Checkbox } from "./ui/checkbox/index.js";

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
      toast(`Created **${dest}**, a git repository. Edit the QML, then try it with ewe-plugin dev or push it.`, "success", 9000);
      try { await openPath(dest); } catch {}
      creating = false; newId = ""; newName = "";
    });
  }
  // settings: typed by the manifest; the CLI refuses what does not fit
  let pending = {};
  function setSetting(p, key, value) {
    clearTimeout(pending[p.id + key]);
    pending[p.id + key] = setTimeout(() => run(p.id, () => api.pluginSet(p.id, key, value), `Saved ${key} for **${p.name || p.id}**`), 350);
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

  function askConfirm(key) {
    confirming = key;
    setTimeout(() => { if (confirming === key) confirming = null; }, 4000);
  }
  function remove(p) {
    if (confirming !== p.id) return askConfirm(p.id);
    confirming = null;
    run(p.id, () => api.pluginRemove(p.id), p.bundled ? `Removed **${p.name || p.id}**. ewe updates leave it out; ewe-plugin seed --restore ${p.id} brings it back.` : `Removed **${p.name || p.id}**`);
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

<Page title="Plugins" desc="Bar widgets, desktop widgets, panels and services for the shell, from a git URL into ~/.config/ewe/plugins">
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

    <Group title="Installed · {installed.length}">
      <svelte:fragment slot="action">
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-expanded={creating} on:click={() => (creating = !creating)}>
          {#if creating}Cancel{:else}<Icon name="plus" />New plugin…{/if}
        </button>
      </svelte:fragment>
      {#if installed.length === 0}
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="puzzle" /></span>
          <div class="ewe-empty__title">No plugins yet</div>
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
                {#if (p.settingsSchema && p.settingsSchema.length) || p.widget}
                  <button
                    class="ewe-btn ewe-btn--sm ewe-btn--secondary"
                    aria-expanded={expanded === p.id}
                    on:click={() => (expanded = expanded === p.id ? null : p.id)}
                  >
                    Options<Icon name={expanded === p.id ? "caretUp" : "caretDown"} />
                  </button>
                {/if}
                <Toggle on={p.enabled} disabled={!p.valid} label="{p.name || p.id} on" toggled={() => setEnabled(p, !p.enabled)} />
              {/if}
            </div>
          </div>
          {#if expanded === p.id}
            <div class="plugin-options" role="group" aria-label="Options of {p.name || p.id}">
              {#if p.widget}
                <Row title="On the desktop" sub="At {p.widget.x}, {p.widget.y}{p.widget.output ? ' on ' + p.widget.output : ''}" dense>
                  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => run(p.id, () => api.pluginArrange(), "Arrange mode: drag widgets on the desktop, then press Esc")}>Arrange…</button>
                </Row>
                <Row title="Above windows" sub="Sticky: the widget stays on top" dense>
                  <Toggle on={p.widget.layer === "top"} label="Above windows" toggled={() => run(p.id, () => api.pluginPlace(p.id, p.widget.layer === "top" ? "desktop" : "top", null), "Saved")} />
                </Row>
                <Row title="Shown" dense>
                  <Toggle on={p.widget.visible !== false} label="Shown" toggled={() => run(p.id, () => api.pluginPlace(p.id, null, p.widget.visible === false), "Saved")} />
                </Row>
              {/if}
              {#each p.settingsSchema || [] as st (st.key)}
                <Row title={st.label || st.key} dense>
                  {#if st.type === "bool"}
                    <Toggle on={!!p.settings[st.key]} label={st.label || st.key} toggled={() => setSetting(p, st.key, !p.settings[st.key])} />
                  {:else if st.type === "int"}
                    <input class="ewe-input num-input ver" type="number" aria-label={st.label || st.key} min={st.min} max={st.max} value={p.settings[st.key]} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                  {:else if st.type === "choice"}
                    <select class="ewe-input text-input" aria-label={st.label || st.key} value={p.settings[st.key]} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)}>
                      {#each st.choices || [] as c}<option value={c}>{c}</option>{/each}
                    </select>
                  {:else if st.type === "color"}
                    <input type="color" class="ewe-swatch" aria-label={st.label || st.key} value={p.settings[st.key]} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                  {:else}
                    <input class="ewe-input text-input" aria-label={st.label || st.key} value={p.settings[st.key] ?? ""} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                  {/if}
                </Row>
              {/each}
            </div>
          {/if}
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
