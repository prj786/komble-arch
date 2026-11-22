<script>
  import { onMount } from "svelte";
  import { settings, systemInfo, toast } from "../stores";
  import { saveSettings } from "../persist";
  import * as api from "../api";
  import ToggleRow from "./ui/ToggleRow.svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Icon from "./ui/Icon.svelte";

  // Local editable copy; re-synced whenever the store changes (load/save).
  let s = { ...$settings };
  $: s = { ...$settings };

  // support: the manifest exactly as the mirror would write it (never written here)
  async function copyManifest() {
    try {
      const text = JSON.stringify(await api.manifestDump(), null, 2);
      await navigator.clipboard.writeText(text);
      toast("Copied the manifest", "success");
    } catch (e) {
      toast(`Couldn’t copy the manifest: ${e}`, "error");
    }
  }

  async function save() {
    try {
      await saveSettings({ ...s });
      toast("Saved the settings", "success");
    } catch (e) {
      toast(e, "error");
    }
  }

  let rechecking = false;
  async function recheck() {
    rechecking = true;
    try {
      await recheck1();
    } finally {
      rechecking = false;
    }
  }
  async function recheck1() {
    readEep();
    try {
      systemInfo.set(await api.systemCheck());
    } catch (e) {
      toast(e, "error");
    }
  }

  // eep, only where it is on PATH: `eep doctor` boiled down to what would
  // stop an install (eep.rs) — { ready, problems }
  let eep = null;
  async function readEep() {
    try {
      const r = await api.eepDoctor();
      eep = r.available ? r : null;
    } catch {
      eep = null;
    }
  }
  onMount(readEep);

  // Tray support is only a question on GNOME, which hides tray icons until
  // its extension is installed. On ewe the bar shows them, so off GNOME the
  // row (and its alert below) would only talk about another desktop.
  $: checks = $systemInfo
    ? [
        ["FUSE for AppImages (fuse2)", $systemInfo.fuse2],
        ["PolicyKit (pkexec)", $systemInfo.pkexec],
        ["pacman", $systemInfo.pacman],
        ["Privileged helper", $systemInfo.helperInstalled],
        ...($systemInfo.gnome ? [["Tray support (GNOME extension)", $systemInfo.appindicatorOk]] : [])
      ]
    : [];

  async function fixFuse() {
    try {
      toast("Installing **fuse2**. You may be asked for your password…", "info");
      await api.installFuse2();
      toast("Installed **fuse2**", "success");
      recheck();
    } catch (e) {
      toast(e, "error");
    }
  }
</script>

<Page title="Settings" desc="How Komble behaves, where it gets apps from, and what it needs from the system">
  <!-- No Appearance group: Komble wears the desktop's scheme, accent, look
       presets and accessibility modes (lib/theme.js), set in ewe's Settings. -->
  <!-- No "Minimize to tray on close": Komble has no tray and quits when its
       window closes (a transaction in flight finishes first). The bar's
       update indicator is the desktop's own and needs no Komble running. -->
  <Group title="Behavior">
    <ToggleRow
      title="Notify about updates"
      sub="Checks every 6 hours while Komble is open"
      on={!!s.notifyUpdates}
      toggled={() => (s.notifyUpdates = !s.notifyUpdates)}
    />
  </Group>

  <Group title="Sources">
    <Row title="GitHub token" sub="Optional. Raises GitHub’s limit for version checks from 60 to 5,000 requests an hour.">
      <label class="ewe-input text-input">
        <Icon name="key" />
        <input class="field-text" type="password" aria-label="GitHub token" placeholder="ghp_…" bind:value={s.githubToken} />
      </label>
    </Row>
    <Row title="AppImage folder" sub="Where AppImages are installed. Empty means ~/.local/share/appimages.">
      <label class="ewe-input text-input">
        <Icon name="folder" />
        <input class="field-text" type="text" aria-label="AppImage folder" placeholder="~/.local/share/appimages" bind:value={s.appimageDir} />
      </label>
    </Row>
    <svelte:fragment slot="after">
      <div class="group-foot">
        <p class="note">The switches above and these two fields are kept when you save.</p>
        <button class="ewe-btn ewe-btn--primary" on:click={save}>Save settings</button>
      </div>
    </svelte:fragment>
  </Group>

  <Group title="System">
    <svelte:fragment slot="action">
      <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={rechecking} on:click={recheck}>
        <Icon name="refresh" />{rechecking ? "Checking…" : "Check again"}
      </button>
    </svelte:fragment>
    {#if $systemInfo}
      {#each checks as [label, ok]}
        <Row title={label}>
          {#if label.startsWith("FUSE") && !ok}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={fixFuse}>Install fuse2</button>
          {/if}
          <span class="ewe-badge {ok ? 'ewe-badge--success' : 'ewe-badge--danger'}">
            <Icon name={ok ? "check" : "x"} size="xs" /><span class="ewe-badge__label">{ok ? "Available" : "Missing"}</span>
          </span>
        </Row>
      {/each}
      {#if eep}
        <Row title="eep apps" sub={eep.problems.join(" ")}>
          <span class="ewe-badge {eep.ready ? 'ewe-badge--success' : 'ewe-badge--warning'}">
            <Icon name={eep.ready ? "check" : "warning"} size="xs" /><span class="ewe-badge__label">{eep.ready ? "Ready" : "Not ready"}</span>
          </span>
        </Row>
      {/if}
    {:else}
      <Row title="Checking the system…">
        <span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>
      </Row>
    {/if}
    <svelte:fragment slot="after">
      {#if $systemInfo && !$systemInfo.helperInstalled}
        <p class="note">
          Without the helper, package actions call pkexec and pacman directly: one password prompt per action. The PKGBUILD installs the helper and its polkit policy.
        </p>
      {/if}
      {#if $systemInfo && $systemInfo.gnome && !$systemInfo.appindicatorOk}
        <div class="ewe-alert ewe-alert--warning" role="alert">
          <Icon name="warning" />
          <div class="ewe-alert__body">
            <div class="ewe-alert__title">GNOME hides tray icons</div>
            <div class="ewe-alert__desc">
              Install the extension with <code>sudo pacman -S gnome-shell-extension-appindicator</code>, then sign out and back in.
            </div>
          </div>
        </div>
      {/if}
    </svelte:fragment>
  </Group>

  <Group title="Advanced">
    <ToggleRow
      title="Show advanced options"
      sub="For people who know what they’re doing"
      on={!!s.advancedMode}
      toggled={() => {
        s.advancedMode = !s.advancedMode;
        saveSettings({ advancedMode: s.advancedMode });
      }}
    />
    {#if s.advancedMode}
      <Row title="Copy manifest" sub="What Komble writes into ewe.conf’s [apps.installed], for a bug report">
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={copyManifest}><Icon name="copy" />Copy</button>
      </Row>
      <Row title="Extra repositories" sub="Planned: adding third-party repositories writes pacman.conf and imports their signing keys. Until then, use the AUR view for packages outside your repositories." dim />
    {/if}
  </Group>
</Page>
