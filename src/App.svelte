<script>
  // The look, live. tokens.css is compiled in as the fallback, and it is built
  // from the DEFAULT accent — so on its own the app wears the wrong greys the
  // moment the user picks an accent. These values come from `ewe-theme show`,
  // which reads THIS machine's ewe.conf, so the whole derived set lands:
  // the brand ramp, and the neutrals carrying the accent's tint.
  //
  // Keyed on the whole THEME INPUT — accent, corner, density, stroke and
  // neutral tint — not on the accent alone. Keying on the accent meant that
  // changing the corner radius or the density rewrote ewe.conf and moved the
  // shell, then hit this guard, returned early, and never reached the app:
  // "shape and density need a restart" was this one comparison.
  let injectedKey = "";
  async function applyThemeTokens(themeKey) {
    const key = String(themeKey || "");
    if (key === injectedKey) return;
    injectedKey = key;
    try {
      const t = await api.themeTokens("ewe");
      if (!t || !t.css_vars) return;
      for (const [k, v] of Object.entries(t.css_vars)) {
        document.documentElement.style.setProperty(k, v);
      }
    } catch {
      injectedKey = "";   // let a later attempt retry
      /* ewe-theme absent (dev, or ewe not deployed) — the compiled-in
         tokens.css already carries the default look */
    }
  }

  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    route,
    installed,
    progress,
    settings,
    systemInfo,
    updatesInfo,
    selectedApp,
    dragHover,
    wizardFile,
    droppedPkg,
    toast
  } from "./lib/stores";
  import { initSettings, saveSettings } from "./lib/persist";
  import { loadCatalog, refreshInstalled, refreshPkgs } from "./lib/actions";
  import * as api from "./lib/api";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Discover from "./lib/components/Discover.svelte";
  import ForYou from "./lib/components/ForYou.svelte";
  import Installed from "./lib/components/Installed.svelte";
  import Updates from "./lib/components/Updates.svelte";
  import AurInstall from "./lib/components/AurInstall.svelte";
  import Settings from "./lib/components/Settings.svelte";
  import AppDetail from "./lib/components/AppDetail.svelte";
  import InstallWizard from "./lib/components/InstallWizard.svelte";
  import Toasts from "./lib/components/Toasts.svelte";

  // Always dark. ewe is dark-only by decision (2026-09-01) and Komble paints
  // itself from the DE's tokens, so a light mode was a look nothing else on
  // the system had — half-themed, and the Settings picker that offered it has
  // gone with it. Set once here, like ewe-settings does.
  document.documentElement.classList.add("dark");

  onMount(() => {
    let unlisteners = [];
    (async () => {
      // "Start at login" is gone: Komble is part of the desktop, not an app
      // you opt into, and the bar's Komble indicator is there whether or not
      // this process is. Clear anything a previous version enabled — leaving
      // it would be a setting with no UI left to turn it off.
      import("@tauri-apps/plugin-autostart")
        .then((m) => m.disable())
        .catch(() => {});

      await initSettings();
      loadCatalog();
      refreshInstalled();
      refreshPkgs();

      // Follow the DE live: accent/scheme/surface changes in Settings land
      // here without a restart — re-read on focus and on a light poll, same
      // "part of the desktop, not a visitor" rule as the first read.
      const applyDePrefs = () =>
        api
          .dePrefs()
          .then((p) => {
            if (!p) return;
            // "" = never picked, so the theme default in tokens.css stands
            if (p.accent) document.documentElement.style.setProperty("--accent", p.accent);
            else document.documentElement.style.removeProperty("--accent");
            // everything ewe-theme derives its token set FROM, as one key
            applyThemeTokens([
              p.accent || "",
              p.themeCorner || "",
              p.themeDensity || "",
              p.themeStroke || "",
              p.neutralTint === undefined ? "" : String(p.neutralTint)
            ].join("|"));
          })
          .catch(() => {});
      applyDePrefs();
      window.addEventListener("focus", applyDePrefs);
      const deTimer = setInterval(applyDePrefs, 4000);
      unlisteners.push(() => { window.removeEventListener("focus", applyDePrefs); clearInterval(deTimer); });

      api
        .systemCheck()
        .then((info) => {
          systemInfo.set(info);
          if (info.gnome && !info.appindicatorOk && !get(settings).hintShown) {
            toast(
            "Heads up: tray icons on GNOME need the “AppIndicator and KStatusNotifier Support” extension (sudo pacman -S gnome-shell-extension-appindicator, then re-log).",
            "info",
              15000
            );
            saveSettings({ hintShown: true });
          }
          if (!info.fuse2) {
            toast(
            "fuse2 is missing — AppImages may not launch. See Settings → System to fix.",
            "info",
              10000
            );
          }
        })
        .catch(() => {});

      // Silent update check so the sidebar badge is useful right away.
      api
        .listUpgradable()
        .then((packages) => updatesInfo.update((u) => ({ ...u, packages })))
        .catch(() => {});

      unlisteners.push(
        await listen("install-progress", (e) => {
          const p = e.payload;
          if (p.phase === "done") {
            progress.update((m) => {
              const c = { ...m };
              delete c[p.id];
              return c;
            });
            refreshInstalled();
          } else {
            progress.update((m) => ({ ...m, [p.id]: p }));
          }
        })
      );
      unlisteners.push(await listen("navigate", (e) => route.set(e.payload)));

      // Files opened WITH Komble (double-click / "Open with" in the file
      // manager) — same destinations as drag & drop.
      const openPath = (p) => {
        if (/\.appimage$/i.test(p)) {
          wizardFile.set(p);
        } else if (p.toLowerCase().includes(".pkg.tar")) {
          droppedPkg.set(p);
          route.set("aur");
        }
      };
      // already running: the second instance forwards its argv as an event
      unlisteners.push(await listen("open-file", (e) => openPath(e.payload)));
      // cold start: the path was stashed before the webview existed
      api.takePendingOpen().then((p) => p && openPath(p)).catch(() => {});
      // `komble --updates` from the DE bar's indicator (cold start)
      api.takePendingRoute().then((r) => r && route.set(r)).catch(() => {});

      // Global drag & drop: .AppImage opens the install wizard,
      // a built/downloaded package file jumps to the install view.
      unlisteners.push(
        await getCurrentWebview().onDragDropEvent((e) => {
          if (e.payload.type === "over") {
            dragHover.set(true);
          } else if (e.payload.type === "drop") {
            dragHover.set(false);
            const paths = e.payload.paths || [];
            const ai = paths.find((p) => /\.appimage$/i.test(p));
            const pkgFile = paths.find((p) => p.toLowerCase().includes(".pkg.tar"));
            if (ai) {
              wizardFile.set(ai);
            } else if (pkgFile) {
              droppedPkg.set(pkgFile);
              route.set("aur");
            } else if (paths.length) {
              toast("Drop an .AppImage or a *.pkg.tar.zst to install it.", "info");
            }
          } else {
            dragHover.set(false);
          }
        })
      );
    })();
    return () => unlisteners.forEach((u) => u());
  });
</script>

<div class="surface flex h-full">
  <Sidebar />
  <!-- .pane: the content well recessed into the frame; overflow stays hidden
       because every page scrolls itself (Discover's grid is windowed). -->
  <main class="pane overflow-hidden">
    {#if $route === "discover"}
      <Discover />
    {:else if $route === "foryou"}
      <ForYou />
    {:else if $route === "installed"}
      <Installed />
    {:else if $route === "updates"}
      <Updates />
    {:else if $route === "aur"}
      <AurInstall />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

{#if $dragHover}
  <div class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-[color-mix(in_srgb,var(--bg-5)_50%,transparent)] p-8 backdrop-blur-sm">
    <!-- the drop zone is a brand-tinted fill, not a dashed outline -->
    <div
      class="flex h-full w-full flex-col items-center justify-center gap-3 rounded-[var(--radius-panel)] bg-[color-mix(in_srgb,var(--brand-bg)_40%,transparent)]"
    >
      <svg viewBox="0 0 24 24" class="h-12 w-12 text-fg" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
      </svg>
      <p class="text-lg font-semibold text-fg">Drop to install</p>
      <p class="text-sm text-muted">.AppImage → install wizard · *.pkg.tar.zst → package installer</p>
    </div>
  </div>
{/if}

{#if $selectedApp}
  <AppDetail />
{/if}
<InstallWizard />
<Toasts />
