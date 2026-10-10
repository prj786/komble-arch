<script>
  // A plugin's options, in a Dialog (design/system/components/Dialog, lg).
  // They used to open inline — for a first-party plugin under the WHOLE card
  // grid, usually off-screen, so a click on Options seemed to do nothing.
  // Three groups, each only when it applies:
  //   Settings        what the manifest declares (`ewe-plugin set`)
  //   In the bar      Show in bar, the host's switch (`ewe-plugin bar`)
  //   On the desktop  a desktop widget's pin, pin level, lock, visibility and
  //                   position (`ewe-plugin place`)
  // Every change applies at once (the shell re-reads live), so the dialog has
  // no Save: Done closes it. `p` is looked up again after every re-read, so it
  // always shows what the CLI last answered. `run` and `setSetting` are the
  // page's (one busy flag, one write chain).
  import { Dialog } from "bits-ui";
  import * as api from "../api";
  import Toggle from "./ui/Toggle.svelte";
  import Row from "./ui/Row.svelte";
  import Seg from "./ui/Seg.svelte";
  import Group from "./ui/Group.svelte";
  import Icon from "./ui/Icon.svelte";
  import * as Select from "./ui/select/index.js";
  import { themeIcon } from "./ui/icons.js";

  export let p;
  export let run;
  export let setSetting;
  export let close = () => {};

  $: name = p.name || p.id;
  $: schema = p.settingsSchema || [];
  $: bar = p.bar && p.bar.toggle !== false ? p.bar : null;
  $: w = p.widget;
  // "small" → "Small", "1password" → "1password"
  const choiceLabel = (c) => {
    const s = String(c);
    return /^[a-z]/.test(s) ? s[0].toUpperCase() + s.slice(1) : s;
  };
  const place = (opts, msg) => run(p.id, () => api.pluginPlace(p.id, opts), msg);
  const PIN_LEVELS = [
    ["top", "Above windows"],
    ["overlay", "Above everything"]
  ];
</script>

<Dialog.Root open={true} onOpenChange={(v) => !v && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="scrim" />
    <!-- the scrim does nothing (Dialog card): Esc, the ✕ and Done close it -->
    <Dialog.Content class="ewe-dialog ewe-dialog--lg is-floating detail plugin-dialog" interactOutsideBehavior="ignore">
      <div class="ewe-dialog__head">
        <span class="ewe-dialog__icon">
          {#if p.icon}<Icon code={themeIcon(p.icon)} />{:else}<Icon name="puzzle" />{/if}
        </span>
        <div class="ewe-dialog__titles">
          <Dialog.Title class="ewe-dialog__title">{name}</Dialog.Title>
          <Dialog.Description class="ewe-dialog__desc">Options. Changes apply right away.</Dialog.Description>
        </div>
        <Dialog.Close class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Close"><Icon name="x" /></Dialog.Close>
      </div>

      <div class="detail__body">
        {#if schema.length}
          <Group title="Settings">
            {#each schema as st (st.key)}
              <Row title={st.label || st.key} sub={st.description || ""}>
                {#if st.type === "bool"}
                  <Toggle on={!!p.settings[st.key]} label={st.label || st.key} toggled={() => setSetting(p, st.key, !p.settings[st.key])} />
                {:else if st.type === "int"}
                  <input class="ewe-input num-input ver" type="number" aria-label={st.label || st.key} min={st.min} max={st.max} value={p.settings[st.key]} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                {:else if st.type === "choice"}
                  {#if (st.choices || []).length <= 3}
                    <Seg label={st.label || st.key} options={(st.choices || []).map((c) => [c, choiceLabel(c)])} value={p.settings[st.key]} picked={(v) => setSetting(p, st.key, v)} />
                  {:else}
                    <Select.Root type="single" value={String(p.settings[st.key] ?? "")} onValueChange={(v) => setSetting(p, st.key, v)}>
                      <Select.Trigger class="select-trigger" aria-label={st.label || st.key}>{choiceLabel(p.settings[st.key] ?? "—")}</Select.Trigger>
                      <Select.Content>
                        {#each st.choices || [] as c (String(c))}
                          <Select.Item value={String(c)} label={choiceLabel(c)} />
                        {/each}
                      </Select.Content>
                    </Select.Root>
                  {/if}
                {:else if st.type === "color"}
                  <input type="color" class="ewe-swatch" aria-label={st.label || st.key} value={p.settings[st.key]} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                {:else}
                  <input class="ewe-input text-input" aria-label={st.label || st.key} value={p.settings[st.key] ?? ""} on:change={(e) => setSetting(p, st.key, e.currentTarget.value)} />
                {/if}
              </Row>
            {/each}
          </Group>
        {/if}

        {#if bar}
          <Group title="In the bar">
            <Row title="Show in bar" sub="Its button or indicator in the top bar. Off hides it there; the plugin keeps working.">
              <Toggle on={bar.shown !== false} label="Show {name} in the bar" toggled={() => run(p.id, () => api.pluginBar(p.id, bar.shown === false), bar.shown === false ? `**${name}** is back in the bar` : `**${name}** is hidden from the bar`)} />
            </Row>
          </Group>
        {/if}

        {#if w}
          <Group title="On the desktop">
            <Row title="Pinned" sub="Keeps it above your windows. The pin on the widget does the same; drag the widget's handle to move it.">
              <Toggle on={!!w.pinned} label="Pin {name}" toggled={() => place({ pinned: !w.pinned }, w.pinned ? `**${name}** is back on the desktop` : `Pinned **${name}**`)} />
            </Row>
            <Row title="When pinned" sub={w.pin_level === "overlay" ? "Above everything, fullscreen apps and games too." : "Above your windows; a fullscreen app still covers it."}>
              <Seg label="When pinned" options={PIN_LEVELS} value={w.pin_level || "top"} picked={(v) => place({ pinLevel: v }, "Saved")} />
            </Row>
            <Row title="Lock position" sub="Stops dragging, so a click never moves it by accident.">
              <Toggle on={!!w.locked} label="Lock {name} in place" toggled={() => place({ locked: !w.locked }, w.locked ? "Unlocked" : "Locked in place")} />
            </Row>
            <Row title="Shown">
              <Toggle on={w.visible !== false} label="Show {name} on the desktop" toggled={() => place({ visible: w.visible === false }, "Saved")} />
            </Row>
            <Row title="Position" sub="At {w.x}, {w.y}{w.output ? ' on ' + w.output : ''}">
              <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" on:click={() => place({ reset: true }, "Back where it started")}>Reset</button>
              <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => run(p.id, () => api.pluginArrange(), "Arrange mode: drag widgets on the desktop, then press Esc")}>Arrange…</button>
            </Row>
          </Group>
        {/if}
      </div>

      <div class="ewe-dialog__foot">
        <Dialog.Close class="ewe-btn ewe-btn--primary">Done</Dialog.Close>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
