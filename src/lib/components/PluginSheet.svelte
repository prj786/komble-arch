<script>
  /**
   * One plugin's sheet (Sheet card, from the right; the list stays usable
   * beside it). It replaces the Options… dialog and is the plugin's whole
   * detail: on/off, what it is and does, where it comes from, what it adds
   * and needs, its options, and Update / Remove — or Install when it is not
   * here yet. Every change applies at once (the shell re-reads live), so
   * there is no Save.
   *
   * `e` is the page's entry for the plugin (Plugins.svelte → entries), looked
   * up again after every re-read, so it always shows what ewe-plugin last
   * answered. The actions are the page's (one busy flag, one write chain):
   *   options   the manifest's `settings`         ewe-plugin set <id> <key> <v>
   *   bar       Show in the bar (host switch)     ewe-plugin bar <id> on|off
   *   desktop   Show on the desktop + placement   ewe-plugin place <id> …
   */
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import Sheet from "./ui/Sheet.svelte";
  import Toggle from "./ui/Toggle.svelte";
  import Row from "./ui/Row.svelte";
  import Seg from "./ui/Seg.svelte";
  import Icon from "./ui/Icon.svelte";
  import Alert from "./ui/Alert.svelte";
  import * as Select from "./ui/select/index.js";

  export let e;
  export let busy = "";
  export let confirming = null;
  export let run;
  export let setSetting;
  export let setEnabled;
  export let install;
  export let update;
  export let remove;
  export let close = () => {};

  // a plugin's kinds in plain words (docs/PLUGINS.md "Kinds")
  const KIND_WORDS = {
    "bar-widget": "Readout in the bar",
    "bar-status": "Status icon in the bar",
    "desktop-widget": "Desktop widget",
    "quick-tile": "Quick settings tile",
    "quick-page": "Page in Quick settings",
    "dock-item": "Button in the dock",
    panel: "Panel",
    overlay: "Screen overlay",
    menu: "Menu",
    service: "Works in the background"
  };
  const PIN_LEVELS = [
    ["top", "Above windows"],
    ["overlay", "Above everything"]
  ];
  // "small" → "Small", "1password" → "1password", "MB/s" stays
  const choiceLabel = (c) => {
    const s = String(c);
    return /^[a-z]/.test(s) ? s[0].toUpperCase() + s.slice(1) : s;
  };
  // https://github.com/acme/ewe-clock.git → github.com/acme/ewe-clock
  const shortUrl = (u) => String(u || "").replace(/^(https?:\/\/|ssh:\/\/|git@)/, "").replace(/\.git$/, "").replace(/^([^/:]+):/, "$1/");

  $: name = e.name;
  $: live = e.live;
  $: on = e.state === "installed" && e.enabled && e.valid;
  $: working = busy === e.id;
  $: schema = (live && live.settingsSchema) || [];
  $: values = (live && live.settings) || {};
  $: bar = live && live.bar && live.bar.toggle !== false ? live.bar : null;
  $: w = live ? live.widget : null;
  $: hasOptions = !!(schema.length || bar || w);
  $: thirdParty = e.where !== "ewe";
  $: from =
    e.where === "ewe"
      ? "From ewe"
      : e.where === "git"
        ? shortUrl(e.source) || "From a git address"
        : e.where === "linked"
          ? "Linked for development"
          : e.where === "gone"
            ? "Not on this machine"
            : "Hand-made";
  $: subtitle = [e.version, from].filter(Boolean).join(" · ");
  $: link = /^https:\/\//.test(e.source || "") ? e.source : /^https:\/\//.test(e.homepage || "") ? e.homepage : "";
  $: needPkgs = e.requires?.packages || [];
  $: needCmds = e.requires?.commands || [];
  $: missingSet = new Set([...(e.missing?.packages || []), ...(e.missing?.commands || [])]);
  $: armed = confirming === e.id;

  const place = (opts, msg) => run(e.id, () => api.pluginPlace(e.id, opts), msg);
  const showInBar = () =>
    run(e.id, () => api.pluginBar(e.id, bar.shown === false), bar.shown === false ? `**${name}** is back in the bar` : `**${name}** is hidden from the bar`);
</script>

<Sheet open={true} title={name} {subtitle} keepOpen=".plugin-row" initialFocus=".ewe-sheet__body :is(button, input, [role='switch']), .ewe-sheet__foot button" onClose={close}>
  <span slot="lead" class="ewe-row__lead ewe-row__lead--tile sheet-tile" class:is-on={on} aria-hidden="true"><Icon code={e.icon} /></span>

  {#if e.state === "installed"}
    <div class="ewe-switch-row">
      <span class="switch-text">
        <span class="switch-text__title">On</span>
        <span class="switch-text__desc">{e.valid ? "Off hides it everywhere; it stays installed" : "It doesn’t load, so it can’t be turned on"}</span>
      </span>
      {#if working}
        <span class="busy" role="status"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Working…</span>
      {:else}
        <Toggle on={e.enabled} disabled={!e.valid || !!busy} label="{name} on" toggled={(v) => setEnabled(e, v)} />
      {/if}
    </div>
  {:else if e.state === "missing"}
    <p class="sheet-text">{e.enabled ? "It was on" : "It was off"} on your other machine and isn’t installed here.</p>
  {/if}

  {#if !e.valid}
    <Alert tone="danger" title="This plugin doesn’t load">
      {#each e.problems.length ? e.problems : ["Its manifest doesn’t validate."] as pr}<div>{pr}</div>{/each}
    </Alert>
  {/if}

  {#if e.description}
    <p class="sheet-text">{e.description}</p>
  {/if}

  {#if e.kinds.length}
    <div class="sheet-tags">
      <span class="sheet-tags__label">Adds</span>
      {#each e.kinds as k (k)}<span class="ewe-tag">{KIND_WORDS[k] || k}</span>{/each}
    </div>
  {/if}

  {#if needPkgs.length || needCmds.length}
    <div class="sheet-tags">
      <span class="sheet-tags__label">Needs</span>
      {#each [...needPkgs, ...needCmds.filter((c) => !needPkgs.includes(c))] as n (n)}
        {#if missingSet.has(n)}
          <span class="ewe-badge ewe-badge--warning" title="Not on this machine yet: installed first, with your password"><span class="ewe-badge__label">{n}</span></span>
        {:else}
          <span class="ewe-tag ver">{n}</span>
        {/if}
      {/each}
    </div>
    {#if missingSet.size && e.state !== "installed"}
      <p class="sheet-note">Install puts in what is missing first. You may be asked for your password.</p>
    {/if}
  {/if}

  {#if thirdParty}
    <!-- the trust model where it applies: installing never runs the code,
         turning it on does -->
    <Alert tone="warning" title="Not from ewe">
      {e.source && e.where === "git" ? `It comes from ${e.source}.` : e.where === "gone" && e.source ? `It came from ${e.source}.` : "It was copied in from a folder."}
      Its code runs unsandboxed inside your shell while it is on, with everything the desktop can do. Read it before you turn it on.
      <svelte:fragment slot="actions">
        {#if link}
          <button class="ewe-link" on:click={() => openUrl(link)}>Open the source<Icon name="external" /></button>
        {/if}
      </svelte:fragment>
    </Alert>
  {/if}

  {#if e.state === "installed"}
    <div class="sheet-rule" role="separator"></div>
    <h3 class="sheet-over">Options</h3>
    {#if hasOptions}
      <div class="ewe-list">
        {#if bar}
          <Row title="Show in the bar">
            <Toggle on={bar.shown !== false} disabled={!!busy} label="Show {name} in the bar" toggled={showInBar} />
          </Row>
        {/if}
        {#if w}
          <Row title="Show on the desktop">
            <Toggle on={w.visible !== false} disabled={!!busy} label="Show {name} on the desktop" toggled={() => place({ visible: w.visible === false }, w.visible === false ? `**${name}** is on the desktop` : `**${name}** is off the desktop`)} />
          </Row>
        {/if}
        {#each schema as st (st.key)}
          <Row title={st.label || st.key} sub={st.description || ""}>
            {#if st.type === "bool"}
              <Toggle on={!!values[st.key]} label={st.label || st.key} toggled={() => setSetting(live, st.key, !values[st.key])} />
            {:else if st.type === "int"}
              <input class="ewe-input num-input ver" type="number" aria-label={st.label || st.key} min={st.min} max={st.max} value={values[st.key]} on:change={(ev) => setSetting(live, st.key, ev.currentTarget.value)} />
            {:else if st.type === "choice"}
              {#if (st.choices || []).length <= 3}
                <Seg size="sm" label={st.label || st.key} options={(st.choices || []).map((c) => [c, choiceLabel(c)])} value={values[st.key]} picked={(v) => setSetting(live, st.key, v)} />
              {:else}
                <Select.Root type="single" value={String(values[st.key] ?? "")} onValueChange={(v) => setSetting(live, st.key, v)}>
                  <Select.Trigger class="select-trigger" aria-label={st.label || st.key}>{choiceLabel(values[st.key] ?? "—")}</Select.Trigger>
                  <Select.Content>
                    {#each st.choices || [] as c (String(c))}
                      <Select.Item value={String(c)} label={choiceLabel(c)} />
                    {/each}
                  </Select.Content>
                </Select.Root>
              {/if}
            {:else if st.type === "color"}
              <input type="color" class="ewe-swatch" aria-label={st.label || st.key} value={values[st.key]} on:change={(ev) => setSetting(live, st.key, ev.currentTarget.value)} />
            {:else}
              <input class="ewe-input text-input" aria-label={st.label || st.key} value={values[st.key] ?? ""} on:change={(ev) => setSetting(live, st.key, ev.currentTarget.value)} />
            {/if}
          </Row>
        {/each}
      </div>
    {:else}
      <p class="sheet-note">Nothing to set: it works as it is.</p>
    {/if}

    {#if w}
      <h3 class="sheet-over">On the desktop</h3>
      <div class="ewe-list">
        <Row title="Pinned" sub="Keeps it above your windows. The pin on the widget does the same; drag its handle to move it.">
          <Toggle on={!!w.pinned} disabled={!!busy} label="Pin {name}" toggled={() => place({ pinned: !w.pinned }, w.pinned ? `**${name}** is back on the desktop` : `Pinned **${name}**`)} />
        </Row>
        <Row title="When pinned" sub={w.pin_level === "overlay" ? "Above everything, fullscreen apps and games too." : "Above your windows; a fullscreen app still covers it."}>
          <Seg size="sm" label="When pinned" options={PIN_LEVELS} value={w.pin_level || "top"} picked={(v) => place({ pinLevel: v }, "Saved")} />
        </Row>
        <Row title="Lock position" sub="Stops dragging, so a click never moves it by accident.">
          <Toggle on={!!w.locked} disabled={!!busy} label="Lock {name} in place" toggled={() => place({ locked: !w.locked }, w.locked ? "Unlocked" : "Locked in place")} />
        </Row>
        <Row title="Position" sub="At {w.x}, {w.y}{w.output ? ' on ' + w.output : ''}">
          <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => place({ reset: true }, "Back where it started")}>Reset</button>
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => run(e.id, () => api.pluginArrange(), "Arrange mode: drag widgets on the desktop, then press Esc")}>Arrange…</button>
        </Row>
      </div>
    {/if}
  {/if}

  <svelte:fragment slot="footer">
    {#if e.state === "installed"}
      {#if e.where === "git"}
        <button class="ewe-btn ewe-btn--secondary" disabled={!!busy} on:click={() => update(e)}><Icon name="refresh" />Update</button>
      {:else}
        <span class="sheet-foot-note">{e.where === "ewe" ? "Updates come with ewe" : "Remove keeps a backup copy"}</span>
      {/if}
      <span class="sheet-foot-gap"></span>
      <button
        class="ewe-btn {armed ? 'ewe-btn--danger' : 'ewe-btn--ghost is-danger'}"
        disabled={!!busy}
        on:click={() => remove(e)}
      >
        <Icon name="trash" />{armed ? `Remove ${name}` : "Remove"}
      </button>
    {:else if e.state === "available"}
      <span class="sheet-foot-note">Takes a second; the shell restarts</span>
      <span class="sheet-foot-gap"></span>
      <button class="ewe-btn ewe-btn--primary" disabled={!!busy} aria-busy={working} on:click={() => install(e)}>
        {#if working}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Installing…{:else}<Icon name="download" />Install{/if}
      </button>
    {:else}
      <span class="sheet-foot-note">{e.fetchable ? "Cloned from its git address" : e.source === "local" ? "A local folder there: nothing to fetch" : "Comes with the ewe package"}</span>
      <span class="sheet-foot-gap"></span>
      {#if e.fetchable}
        <button class="ewe-btn ewe-btn--primary" disabled={!!busy} on:click={() => install(e)}>
          {#if working}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Cloning…{:else}<Icon name="download" />Install{/if}
        </button>
      {/if}
    {/if}
  </svelte:fragment>
</Sheet>
