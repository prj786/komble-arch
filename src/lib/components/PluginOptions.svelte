<script>
  // The options block of one installed plugin: a desktop widget's placement
  // and the typed settings its manifest declares. Shared by the add-on cards
  // and the plugin rows of the Plugins page; `run` and `setSetting` are the
  // page's (one busy flag, one write chain), so the two never disagree.
  import * as api from "../api";
  import Toggle from "./ui/Toggle.svelte";
  import Row from "./ui/Row.svelte";

  export let p;
  export let run;
  export let setSetting;
  let className = "";
  export { className as class };
</script>

<div class="plugin-options {className}" role="group" aria-label="Options of {p.name || p.id}">
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
