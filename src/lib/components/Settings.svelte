<script>
  import { settings, systemInfo, toast } from "../stores";
  import { saveSettings } from "../persist";
  import * as api from "../api";
  import { enable, disable } from "@tauri-apps/plugin-autostart";

  // Local editable copy; re-synced whenever the store changes (load/save).
  let s = { ...$settings };
  $: s = { ...$settings };

  async function save() {
    try {
      try {
        if (s.autostart) await enable();
        else await disable();
      } catch {
        // autostart plugin may be unavailable in dev; not fatal
      }
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

<div class="h-full overflow-y-auto px-6 py-6">
  <h1 class="mb-4 text-xl font-bold tracking-tight">Settings</h1>
  <div class="max-w-2xl">
    <div class="section-title">Appearance</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <label class="flex items-center justify-between px-4 py-3">
        <span class="text-sm font-medium">Theme</span>
        <select class="input !w-40" bind:value={s.theme} on:change={() => saveSettings({ theme: s.theme })}>
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>
    </div>

    <div class="section-title">Behavior</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <label class="flex items-center justify-between px-4 py-3">
        <div>
          <div class="text-sm font-medium">Minimize to tray on close</div>
          <div class="text-xs text-zinc-400">Closing the window keeps Komble running in the tray</div>
        </div>
        <input type="checkbox" class="h-4 w-4" style="accent-color: var(--accent)" bind:checked={s.minimizeToTray} />
      </label>
      <label class="flex items-center justify-between px-4 py-3">
        <div>
          <div class="text-sm font-medium">Start at login</div>
          <div class="text-xs text-zinc-400">Launch Komble automatically when you log in</div>
        </div>
        <input type="checkbox" class="h-4 w-4" style="accent-color: var(--accent)" bind:checked={s.autostart} />
      </label>
      <label class="flex items-center justify-between px-4 py-3">
        <div>
          <div class="text-sm font-medium">Notify about updates</div>
          <div class="text-xs text-zinc-400">Background check every 6 hours</div>
        </div>
        <input type="checkbox" class="h-4 w-4" style="accent-color: var(--accent)" bind:checked={s.notifyUpdates} />
      </label>
    </div>

    <div class="section-title">Sources</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <label class="block px-4 py-3">
        <div class="text-sm font-medium">GitHub token <span class="font-normal text-zinc-400">(optional)</span></div>
        <div class="mb-2 text-xs text-zinc-400">
          Raises the GitHub API limit from 60 to 5000 requests/hour for version checks
        </div>
        <input type="password" class="input" placeholder="ghp_…" bind:value={s.githubToken} />
      </label>
      <label class="block px-4 py-3">
        <div class="text-sm font-medium">AppImage install directory</div>
        <div class="mb-2 text-xs text-zinc-400">Default: ~/.local/share/appimages</div>
        <input type="text" class="input" placeholder="~/.local/share/appimages" bind:value={s.appimageDir} />
      </label>
    </div>

    <div class="mt-4">
      <button class="btn-primary" on:click={save}>Save settings</button>
    </div>

    <div class="section-title">System</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
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
              <span class="h-2.5 w-2.5 rounded-full {ok ? 'bg-green-500' : 'bg-red-500'}"></span>
            </span>
          </div>
        {/each}
        {#if !$systemInfo.helperInstalled}
          <div class="px-4 py-2.5 text-xs text-zinc-400">
            Without the helper, package actions call pkexec + pacman directly (one auth
            prompt per action). The helper and its polkit policy are installed by the
            PKGBUILD.
          </div>
        {/if}
        {#if $systemInfo.gnome && !$systemInfo.appindicatorOk}
          <div class="px-4 py-2.5 text-xs text-amber-600 dark:text-amber-400">
            GNOME hides tray icons by default. Install the extension:
            <code class="rounded bg-zinc-100 px-1 dark:bg-zinc-700">sudo pacman -S gnome-shell-extension-appindicator</code>
            then log out and back in.
          </div>
        {/if}
      {:else}
        <div class="px-4 py-3 text-sm text-zinc-400">Checking system…</div>
      {/if}
      <div class="px-4 py-2.5">
        <button class="btn-ghost !py-1 text-xs" on:click={recheck}>Re-check</button>
      </div>
    </div>

    <div class="section-title">Advanced</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <label class="flex items-center justify-between px-4 py-3">
        <div>
          <div class="text-sm font-medium">Show advanced options</div>
          <div class="text-xs text-zinc-400">For users who know what they're doing</div>
        </div>
        <input
          type="checkbox"
          class="h-4 w-4"
          style="accent-color: var(--accent)"
          bind:checked={s.advancedMode}
          on:change={() => saveSettings({ advancedMode: s.advancedMode })}
        />
      </label>
      {#if s.advancedMode}
        <div class="px-4 py-3 text-sm text-zinc-400">
          <div class="font-medium text-zinc-500 dark:text-zinc-300">Extra repositories</div>
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
