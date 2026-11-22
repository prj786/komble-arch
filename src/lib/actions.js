import * as api from "./api";
import {
  catalog,
  catalogLoading,
  catalogError,
  installed,
  trackedPkgs,
  firstPartyPkgs,
  installedPkgNames,
  eepOn,
  eepApps,
  eepInstalled,
  eepError,
  eepBusy,
  toast
} from "./stores";
import { stripHtml, formatBytes } from "./utils";

export async function loadCatalog(force = false) {
  catalogLoading.set(true);
  catalogError.set("");
  try {
    const items = await api.fetchCatalog(force);
    for (const i of items) i.plainDesc = stripHtml(i.description);
    catalog.set(items);
  } catch (e) {
    catalogError.set(String(e));
  }
  catalogLoading.set(false);
}

export function refreshInstalled() {
  api.listAppimages().then(installed.set).catch(() => {});
}

export function refreshPkgs() {
  api.listTrackedPackages().then(trackedPkgs.set).catch(() => {});
  api.firstPartyInstalled().then(firstPartyPkgs.set).catch(() => {});
  api.installedPackageNames().then((names) => installedPkgNames.set(new Set(names))).catch(() => {});
}

// ── eep apps (eep.rs) ──
// Discover's item for one app of `eep available`: kind "eep", id
// "eep:<id>" (never an AppImage's id), `eep` the id eep itself knows;
// its bytes travel as downloadSize (a catalog item's `download` is a URL)
const eepItem = ({ download, ...a }) => ({
  ...a,
  downloadSize: download,
  kind: "eep",
  id: `eep:${a.id}`,
  eep: a.id,
  appKind: a.kind,
  plainDesc: a.description,
  categories: []
});

// only the newest call's answer lands (an install can finish while an
// older read is still out)
let eepSeq = 0;
/** `eep available` + `eep list` in parallel — both local reads (no Nix
 *  call); `refresh` asks eep's server for a newer app list first. Without
 *  eep on PATH nothing is spawned and the source stays hidden. */
export async function refreshEep(refresh = false) {
  const n = ++eepSeq;
  try {
    const [r, list] = await Promise.all([api.eepAvailable(refresh), api.eepList().catch(() => null)]);
    if (n !== eepSeq) return;
    eepOn.set(!!r.available);
    eepError.set(r.error || "");
    eepApps.set((r.apps || []).map(eepItem));
    if (list) eepInstalled.set(list);
  } catch {
    // a backend without eep: the source stays hidden
  }
}

const EEP_VERBS = {
  install: ["Installing", "Installed", api.eepInstall],
  update: ["Updating", "Updated", api.eepUpdate],
  remove: ["Removing", "Removed", api.eepRemove]
};

const setEepBusy = (keys, verb) =>
  eepBusy.update((m) => {
    const n = { ...m };
    for (const k of keys) {
      if (verb) n[k] = verb;
      else delete n[k];
    }
    return n;
  });

/** Install, update or remove eep apps ([{eep, name}]): busy (eepBusy)
 *  while eep runs, the toasts every source has, eep's own words on a
 *  failure. No password: eep works as you. → true on success */
export async function eepRun(verb, apps) {
  const [doing, done, call] = EEP_VERBS[verb];
  const keys = apps.map((a) => `eep:${a.eep}`);
  const what = apps.length === 1 ? `**${apps[0].name}**` : `${apps.length} apps`;
  setEepBusy(keys, verb);
  let ok = false;
  try {
    toast(`${doing} ${what}…`, "info");
    const r = await call(apps.map((a) => a.eep));
    const size = verb !== "remove" && r && r.download > 0 ? ` (${formatBytes(r.download)} downloaded)` : "";
    toast(`${done} ${what}${size}`, "success");
    ok = true;
  } catch (e) {
    toast(e, "error");
  }
  setEepBusy(keys, null);
  await refreshEep();
  return ok;
}
