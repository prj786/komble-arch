import * as api from "./api";
import {
  catalog,
  catalogLoading,
  catalogError,
  installed,
  trackedPkgs
} from "./stores";
import { stripHtml } from "./utils";

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
}
