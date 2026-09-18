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
  // autostart: removed with the "Start at login" toggle. Kept out of the
  // defaults so it stops being written back; App.svelte clears any entry a
  // previous version created.
  githubToken: "",
  appimageDir: "",
  notifyUpdates: true,
  advancedMode: false,
  hintShown: false
};
export const settings = writable({ ...DEFAULT_SETTINGS });

/** id → { phase, downloaded, total } while an install/update is running. */
export const progress = writable({});

/** Package names (pacman / AUR / first-party) with an install or remove in
 *  flight. ONE shared set — a card and the detail modal must never disagree
 *  about "Installing…" (they used to each keep a private busy flag, so the
 *  modal said "Install" while the card underneath said "Installing"). */
export const busyPkgs = writable(new Set());
export function setPkgBusy(pkg, busy) {
  busyPkgs.update((s) => {
    const n = new Set(s);
    if (busy) n.add(pkg);
    else n.delete(pkg);
    return n;
  });
}

/** Catalog item currently open in the detail modal. */
export const selectedApp = writable(null);

/** True while a file is being dragged over the window (drop overlay). */
export const dragHover = writable(false);
/** Path of a dropped .AppImage → opens the install wizard. */
export const wizardFile = writable(null);
/** Path of a dropped *.pkg.tar.zst → picked up by the AUR/local install view. */
export const droppedPkg = writable(null);
/** AUR package name → the AUR view opens its PKGBUILD review. Installing from
 *  the AUR is never one-click: the review gate is the security model. */
export const aurReview = writable("");
// The rest of a restore's AUR list: after one review+build finishes, the AUR
// view opens the next one ("Review next") so the user walks the list instead
// of hunting each name. Names only; each still gets its own PKGBUILD gate.
export const aurQueue = writable([]);

export const updatesInfo = writable({ appimages: [], packages: [], errors: [], self: null, desktop: 0, checkedAt: 0 });
export const updatesCount = derived(
  updatesInfo,
  (i) => i.appimages.length + i.packages.length + (i.desktop || 0) + (i.self ? 1 : 0)
);

export const systemInfo = writable(null);

export const installedIds = derived(installed, (list) => new Set(list.map((e) => e.id)));

// ---------- toasts ----------

// ── after an upgrade ──
// {level: reboot|logout|shell|komble, reasons: [...], packages: [...]} — the
// RestartDialog shows it; Updates keeps a card until it is acted on
export const restartNeed = writable(null);
// {conflicts: [{keep, remove, reason}], resolve(bool)} — pacman's "Remove Y?"
// question, asked of the person by ConflictDialog instead of failing quietly
export const conflictPrompt = writable(null);
// a search the app was opened INTO (`komble --search=pdf`); Discover consumes it
export const pendingSearch = writable("");

// The Toast (design/system/components/Toast): one at a time, centered at the
// bottom; a new one replaces the current one. 5 s, 8 s with an action or for
// a failure, or `ms`; Toasts.svelte pauses the timer while it is hovered or
// focused. `message` may name the thing in **bold**. The old type names are
// kept at the call sites: "error" is the danger tone.
export const currentToast = writable(null);
let toastId = 0;
const TONES = { error: "danger", danger: "danger", success: "success", warning: "warning", info: "info" };

export function toast(message, type = "info", ms, action = null) {
  const id = ++toastId;
  const tone = TONES[type] || "info";
  const timeout = ms ?? (action || tone === "danger" ? 8000 : 5000);
  currentToast.set({ id, message: String(message), tone, action, timeout });
  return id;
}

export function dismissToast(id) {
  currentToast.update((t) => (t && (id == null || t.id === id) ? null : t));
}
