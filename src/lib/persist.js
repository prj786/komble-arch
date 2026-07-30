import { load } from "@tauri-apps/plugin-store";
import { get } from "svelte/store";
import { settings, DEFAULT_SETTINGS } from "./stores";

let store = null;

/** Settings are stored as top-level keys so the Rust side can read them too. */
export async function initSettings() {
  store = await load("settings.json", { autoSave: false });
  const merged = { ...DEFAULT_SETTINGS };
  for (const key of Object.keys(DEFAULT_SETTINGS)) {
    const v = await store.get(key);
    if (v !== undefined && v !== null) merged[key] = v;
  }
  settings.set(merged);
  return merged;
}

export async function saveSettings(patch = {}) {
  settings.update((s) => ({ ...s, ...patch }));
  if (!store) return;
  const s = get(settings);
  for (const key of Object.keys(DEFAULT_SETTINGS)) {
    await store.set(key, s[key]);
  }
  await store.save();
}
