<script>
  import { settings, systemInfo, toast } from "../stores";
  import { saveSettings } from "../persist";
  import * as api from "../api";
  import ToggleRow from "./ui/ToggleRow.svelte";

  // Local editable copy; re-synced whenever the store changes (load/save).
  let s = { ...$settings };
  $: s = { ...$settings };

  // support: the manifest exactly as the mirror would write it (never written here)
  async function copyManifest() {
    try {
      const text = JSON.stringify(await api.manifestDump(), null, 2);
      await navigator.clipboard.writeText(text);
      toast("Manifest copied to the clipboard.", "success");
    } catch (e) {
      toast(`Could not copy: ${e}`, "error");
    }
  }

  async function save() {
    try {
      await saveSettings({ ...s });
      toast("Settings saved", "success");
    } catch (e) {
      toast(e, "error");
    }
  }

  async function recheck() {
    try {
      systemInfo.set(await api.systemCheck());
    } catch (e) {
      toast(e, "error");
    }
  }

  async function fixFuse() {
    try {
      toast("Installing FUSE compatibility — authentication required…", "info");
      await api.installFuse2();
      toast("fuse2 installed", "success");
      recheck();
    } catch (e) {
      toast(e, "error");
    }
  }
</script>

<div class="h-full overflow-y-auto px-4 py-5 sm:px-6 sm:py-6">
  <h1 class="mb-4 text-xl font-bold tracking-tight">Settings</h1>
  <div class="max-w-2xl">
    <!-- No Appearance section. ewe is dark-only by decision (2026-09-01) and
         Komble follows the DE's tokens, so a Light option offered a look the
         desktop does not have — half-themed, and nothing else on the system
         would have followed it. -->
    <div class="section-title">Behavior</div>
    <div class="card divide-y divide-hairline">
      <ToggleRow
        title="Minimize to tray on close"
        sub="Closing the window keeps Komble running in the tray"
        on={!!s.minimizeToTray}
        toggled={() => (s.minimizeToTray = !s.minimizeToTray)}
      />
      <ToggleRow
        title="Notify about updates"
        sub="Background check every 6 hours"
        on={!!s.notifyUpdates}
        toggled={() => (s.notifyUpdates = !s.notifyUpdates)}
      />
    </div>

    <div class="section-title">Sources</div>
    <div class="card divide-y divide-hairline">
      <label class="block px-4 py-3">
        <div class="text-sm font-medium">GitHub token <span class="font-normal text-dim">(optional)</span></div>
        <div class="mb-2 text-xs text-dim">
          Raises the GitHub API limit from 60 to 5000 requests/hour for version checks
        </div>
        <input type="password" class="input" placeholder="ghp_…" bind:value={s.githubToken} />
      </label>
      <label class="block px-4 py-3">
        <div class="text-sm font-medium">AppImage install directory</div>
        <div class="mb-2 text-xs text-dim">Default: ~/.local/share/appimages</div>
        <input type="text" class="input" placeholder="~/.local/share/appimages" bind:value={s.appimageDir} />
      </label>
    </div>

    <div class="mt-4">
      <button class="btn-primary" on:click={save}>Save settings</button>
    </div>

    <div class="section-title">System</div>
    <div class="card divide-y divide-hairline">
      {#if $systemInfo}
        {#each [
          ["FUSE for AppImages (fuse2)", $systemInfo.fuse2],
          ["PolicyKit (pkexec)", $systemInfo.pkexec],
          ["pacman", $systemInfo.pacman],
          ["Privileged helper installed", $systemInfo.helperInstalled],
          ["Tray support" + ($systemInfo.gnome ? " (GNOME extension)" : ""), $systemInfo.appindicatorOk]
        ] as [label, ok]}
          <div class="flex items-center justify-between px-4 py-2.5">
            <span class="text-sm">{label}</span>
            <span class="flex items-center gap-2">
              {#if label.startsWith("FUSE") && !ok}
                <button class="btn-ghost !py-0.5 text-xs" on:click={fixFuse}>Fix</button>
              {/if}
              <span class="h-2.5 w-2.5 rounded-full {ok ? 'bg-[color-mix(in_srgb,var(--success)_14%,transparent)]0' : 'bg-[color-mix(in_srgb,var(--danger)_14%,transparent)]0'}"></span>
            </span>
          </div>
        {/each}
        {#if !$systemInfo.helperInstalled}
          <div class="px-4 py-2.5 text-xs text-dim">
            Without the helper, package actions call pkexec + pacman directly (one auth
            prompt per action). The helper and its polkit policy are installed by the
            PKGBUILD.
          </div>
        {/if}
        {#if $systemInfo.gnome && !$systemInfo.appindicatorOk}
          <div class="px-4 py-2.5 text-xs text-warning dark:text-warning">
            GNOME hides tray icons by default. Install the extension:
            <code class="rounded bg-elevated px-1 ">sudo pacman -S gnome-shell-extension-appindicator</code>
            then log out and back in.
          </div>
        {/if}
      {:else}
        <div class="px-4 py-3 text-sm text-dim">Checking system…</div>
      {/if}
      <div class="px-4 py-2.5">
        <button class="btn-ghost !py-1 text-xs" on:click={recheck}>Re-check</button>
      </div>
    </div>

    <div class="section-title">Advanced</div>
    <div class="card divide-y divide-hairline">
      <ToggleRow
        title="Show advanced options"
        sub="For users who know what they're doing"
        on={!!s.advancedMode}
        toggled={() => {
          s.advancedMode = !s.advancedMode;
          saveSettings({ advancedMode: s.advancedMode });
        }}
      />
      {#if s.advancedMode}
        <div class="flex items-center justify-between gap-3 px-4 py-3 text-sm">
          <div>
            <div class="font-medium text-dim ">Copy manifest</div>
            <p class="mt-0.5 text-xs text-dim">
              What Komble writes into ewe.conf's [apps.installed] — for a bug report.
            </p>
          </div>
          <button class="btn-ghost !py-1 text-xs" on:click={copyManifest}>Copy</button>
        </div>
        <div class="px-4 py-3 text-sm text-dim">
          <div class="font-medium text-dim ">Extra repositories</div>
          <p class="mt-1 text-xs">
            Planned (phase 2): adding third-party repos writes to
            pacman.conf and imports signing keys into the pacman keyring.
            Until then, use the AUR view for packages outside your repositories.
          </p>
        </div>
      {/if}
    </div>
  </div>
</div>
