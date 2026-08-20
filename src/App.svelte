<script>
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

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  // Inside ewe, "System" follows the DE's app colour scheme (the shell
  // itself is always dark) and the accent follows the DE accent — Komble should
  // look like a part of the desktop, not a visitor. Outside it, the old
  // prefers-color-scheme behaviour stands.
  let deScheme = "";
  function applyTheme(theme) {
    const dark =
      theme === "dark" ||
      (theme === "system" && (deScheme ? deScheme === "dark" : media.matches));
    document.documentElement.classList.toggle("dark", dark);
  }
  $: applyTheme($settings.theme);
  media.addEventListener("change", () => applyTheme(get(settings).theme));

  onMount(() => {
    let unlisteners = [];
    (async () => {
      await initSettings();
      loadCatalog();
      refreshInstalled();
      refreshPkgs();

      api
        .dePrefs()
        .then((p) => {
          if (!p) return;
          deScheme = p.colorScheme || "";
          document.documentElement.style.setProperty("--accent", p.accent);
          document.documentElement.classList.toggle("blacksheep", (p.themeName || "flock") === "blacksheep");
          applyTheme(get(settings).theme);
        })
        .catch(() => {});

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

<div class="flex h-full">
  <Sidebar />
  <main class="min-w-0 flex-1 overflow-hidden">
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
  <div class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-8 backdrop-blur-sm">
    <div
      class="flex h-full w-full flex-col items-center justify-center gap-3 rounded-3xl border-2 border-dashed"
      style="border-color: var(--accent)"
    >
      <svg viewBox="0 0 24 24" class="h-12 w-12 text-white" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
      </svg>
      <p class="text-lg font-semibold text-white">Drop to install</p>
      <p class="text-sm text-white/70">.AppImage → install wizard · *.pkg.tar.zst → package installer</p>
    </div>
  </div>
{/if}

{#if $selectedApp}
  <AppDetail />
{/if}
<InstallWizard />
<Toasts />
