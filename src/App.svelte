<script>
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    route,
    pendingSearch,
    pendingOptions,
    installed,
    progress,
    settings,
    systemInfo,
    updatesInfo,
    selectedApp,
    dragHover,
    wizardFile,
    droppedPkg,
    installPicks,
    toast
  } from "./lib/stores";
  import { initSettings, saveSettings } from "./lib/persist";
  import { loadCatalog, refreshInstalled, refreshPkgs, refreshEep } from "./lib/actions";
  import * as api from "./lib/api";
  import { watchTheme } from "./lib/theme.js";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Discover from "./lib/components/Discover.svelte";
  import ForYou from "./lib/components/ForYou.svelte";
  import Installed from "./lib/components/Installed.svelte";
  import Plugins from "./lib/components/Plugins.svelte";
  import Updates from "./lib/components/Updates.svelte";
  import AurInstall from "./lib/components/AurInstall.svelte";
  import Settings from "./lib/components/Settings.svelte";
  import AppDetail from "./lib/components/AppDetail.svelte";
  import InstallWizard from "./lib/components/InstallWizard.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import ConflictDialog from "./lib/components/ConflictDialog.svelte";
  import RestartDialog from "./lib/components/RestartDialog.svelte";
  import Icon from "./lib/components/ui/Icon.svelte";
  import { Tooltip } from "bits-ui";

  onMount(() => {
    let unlisteners = [];
    // The look, live (lib/theme.js): at start, on window focus, and when the
    // desktop's look record moves — a scheme, the accent, a look preset, an
    // accessibility mode — even while Komble sits unfocused beside Settings.
    unlisteners.push(
      watchTheme(() => api.dePrefs().then((p) => (p ? JSON.stringify(p.look ?? p) : null)))
    );
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
      refreshEep();

      api
        .systemCheck()
        .then((info) => {
          systemInfo.set(info);
          if (info.gnome && !info.appindicatorOk && !get(settings).hintShown) {
            toast(
              "Tray icons on GNOME need the “AppIndicator and KStatusNotifier Support” extension. Settings › System shows how to add it.",
              "info",
              15000
            );
            saveSettings({ hintShown: true });
          }
          if (!info.fuse2) {
            toast(
              "AppImages may not start: fuse2 is missing. Fix it in Settings › System.",
              "warning",
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
      // eep's cheap count (no Nix call); nothing is spawned without eep
      api
        .eepUpdates(true)
        .then((r) => updatesInfo.update((u) => ({ ...u, eepCount: r.count || 0 })))
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
      // "search:<words>" is a route INTO Discover with the box pre-filled
      // (the desktop's gnome-software stand-in: GTK's "Find New Applications")
      const goRoute = (r) => {
        if (typeof r === "string" && r.startsWith("search:")) {
          pendingSearch.set(r.slice(7));
          route.set("discover");
        } else if (typeof r === "string" && r.startsWith("options:")) {
          // a plugin's sheet (its options and the rest), on the Plugins page
          pendingOptions.set(r.slice(8));
          route.set("plugins");
        } else {
          route.set(r);
        }
      };

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
      // Rust stashes the path / the route and says so; the page takes it —
      // on mount (the window was just built for it: a cold start, or a
      // second launch while a transaction kept a windowless Komble alive)
      // or on the event (it was already open), never both.
      const takeOpen = () => api.takePendingOpen().then((p) => p && openPath(p)).catch(() => {});
      const takeRoute = () => api.takePendingRoute().then((r) => { if (r) goRoute(r); return !!r; }).catch(() => false);
      unlisteners.push(await listen("open-file", takeOpen));
      unlisteners.push(await listen("navigate", takeRoute));
      takeOpen();
      // `komble --updates` from the DE bar's indicator; with no route asked
      // for, apps picked while installing ewe open For you (they wait there
      // until installed or dismissed — the rail counts them)
      takeRoute().then(async (routed) => {
        try {
          const r = await api.installPicks();
          installPicks.set(r.available ? r.picked || [] : []);
          if (r.available && !routed && get(route) === "discover") route.set("foryou");
        } catch {
          // an older backend or no file: nothing waits
        }
      });

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
              toast("Drop an .AppImage or a package file (.pkg.tar.zst) to install it.", "info");
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

<Tooltip.Provider>
  <div class="ewe-appwin">
    <Sidebar />
    <!-- the pane: the main landmark; every page scrolls itself (Discover's
         grid is windowed), so the scroll box is the page's own -->
    <main class="ewe-appwin__pane">
      <div class="ewe-appwin__scroll">
        {#if $route === "discover"}
          <Discover />
        {:else if $route === "foryou"}
          <ForYou />
        {:else if $route === "installed"}
          <Installed />
        {:else if $route === "plugins"}
          <Plugins />
        {:else if $route === "updates"}
          <Updates />
        {:else if $route === "aur"}
          <AurInstall />
        {:else}
          <Settings />
        {/if}
      </div>
    </main>
  </div>

  {#if $dragHover}
    <!-- File drop zone, dragged over, across the whole window -->
    <div class="dropzone" aria-hidden="true">
      <div class="ewe-drop is-dragover">
        <Icon name="download" />
        <div class="ewe-drop__title">Drop to install</div>
        <div class="ewe-drop__hint">An .AppImage opens the install wizard; a .pkg.tar.zst opens the package installer.</div>
      </div>
    </div>
  {/if}

  {#if $selectedApp}
    <AppDetail />
  {/if}
  <InstallWizard />
  <Toasts />
  <ConflictDialog />
  <RestartDialog />
</Tooltip.Provider>
