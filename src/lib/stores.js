import { writable, derived } from "svelte/store";

export const route = writable("discover");

export const catalog = writable([]);
export const catalogLoading = writable(true);
export const catalogError = writable("");

/** Installed AppImages (registry entries from the backend). */
export const installed = writable([]);
/** Packages installed through Komble. */
export const trackedPkgs = writable([]);

export const DEFAULT_SETTINGS = {
  theme: "system",
  minimizeToTray: true,
  autostart: false,
  githubToken: "",
  appimageDir: "",
  notifyUpdates: true,
  advancedMode: false,
  hintShown: false
};
export const settings = writable({ ...DEFAULT_SETTINGS });

/** id → { phase, downloaded, total } while an install/update is running. */
export const progress = writable({});

/** Catalog item currently open in the detail modal. */
export const selectedApp = writable(null);

/** True while a file is being dragged over the window (drop overlay). */
export const dragHover = writable(false);
/** Path of a dropped .AppImage → opens the install wizard. */
export const wizardFile = writable(null);
/** Path of a dropped *.pkg.tar.zst → picked up by the AUR/local install view. */
export const droppedPkg = writable(null);

export const updatesInfo = writable({ appimages: [], packages: [], errors: [], self: null, checkedAt: 0 });
export const updatesCount = derived(
  updatesInfo,
  (i) => i.appimages.length + i.packages.length + (i.self ? 1 : 0)
);

export const systemInfo = writable(null);

export const installedIds = derived(installed, (list) => new Set(list.map((e) => e.id)));

// ---------- toasts ----------

export const toasts = writable([]);
let toastId = 0;

export function toast(message, type = "info", ms) {
  const id = ++toastId;
  const timeout = ms ?? (type === "error" ? 8000 : 4000);
  toasts.update((t) => [...t, { id, message: String(message), type }]);
  setTimeout(() => dismissToast(id), timeout);
  return id;
}

export function dismissToast(id) {
  toasts.update((t) => t.filter((x) => x.id !== id));
}
