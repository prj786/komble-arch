<script>
  /**
   * One plugin, one row (List row), the same anatomy in every group and every
   * state: the icon tile, the name and one line, then the trail — what it
   * still needs (before it is installed), its category as a Tag, the one
   * control (the on/off Switch when installed, Install when not), and ⋯ for
   * the rest. Clicking the row opens the plugin's sheet; the name is the
   * button that does it from the keyboard. Controls in the trail never open
   * the sheet.
   *
   * state: installed | available (From ewe, not installed) | missing (from
   * your other machine). menu: [{ label, icon, danger, run }].
   */
  import Toggle from "./ui/Toggle.svelte";
  import Icon from "./ui/Icon.svelte";
  import * as Menu from "./ui/dropdown-menu/index.js";

  export let name = "";
  export let desc = "";
  export let icon = 0;
  export let tag = "";
  export let needs = "";
  export let needsTitle = "";
  export let lit = false;
  export let state = "available";
  export let enabled = false;
  export let broken = false;
  export let busy = false;
  export let selected = false;
  export let canInstall = true;
  export let menu = [];
  export let open = () => {};
  export let toggle = () => {};
  export let install = () => {};
</script>

<!-- the row's own click is a pointer convenience; the name button is the
     keyboard path to the same sheet -->
<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div
  class="ewe-row ewe-row--interactive plugin-row"
  class:is-selected={selected}
  class:is-dim={broken}
  role="listitem"
  on:click={open}
>
  <span class="ewe-row__lead ewe-row__lead--tile" class:is-on={lit}><Icon code={icon} /></span>
  <button type="button" class="ewe-row__text plugin-row__open" aria-haspopup="dialog" on:click|stopPropagation={open}>
    <span class="ewe-row__title">{name}</span>
    {#if desc}<span class="ewe-row__desc" class:text-danger={broken}>{desc}</span>{/if}
  </button>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <span class="ewe-row__trail" on:click|stopPropagation>
    {#if needs}
      <span class="ewe-badge ewe-badge--warning" title={needsTitle || undefined}><span class="ewe-badge__label">{needs}</span></span>
    {/if}
    {#if tag}<span class="ewe-tag">{tag}</span>{/if}
    <span class="plugin-row__ctl">
      {#if busy}
        <span class="busy" role="status"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Working…</span>
      {:else if state === "installed"}
        <Toggle on={enabled} disabled={broken} label="{name} on" toggled={(v) => toggle(v)} />
      {:else if canInstall}
        <button type="button" class="ewe-btn ewe-btn--sm ewe-btn--primary" aria-label="Install {name}" on:click={install}>Install</button>
      {/if}
    </span>
    <span class="plugin-row__more">
      {#if menu.length && !busy}
        <Menu.Root>
          <Menu.Trigger class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="More for {name}">
            <Icon name="more" />
          </Menu.Trigger>
          <Menu.Content align="end">
            {#each menu as m, i (m.label)}
              {#if m.danger && i > 0}<Menu.Separator />{/if}
              <Menu.Item variant={m.danger ? "destructive" : "default"} onSelect={m.run}>
                {#if m.icon}<Icon name={m.icon} />{/if}<span class="ewe-menu__label">{m.label}</span>
              </Menu.Item>
            {/each}
          </Menu.Content>
        </Menu.Root>
      {/if}
    </span>
  </span>
</div>
