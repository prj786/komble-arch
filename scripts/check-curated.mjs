#!/usr/bin/env node
// Validates src/lib/curated.js against the real package sources, so the
// curated "Popular" lists can be re-checked any time the packages drift.
// No dependencies; needs pacman (for repo packages) and network (AUR + AM).
//
//   repo      → `pacman -Si <pkg>` must print a non-empty Repository line
//   aur       → the AUR RPC must answer resultcount 1
//   appimage  → the AM install script (what resolve_am_app fetches) must be 200
//
// Prints one line per app and exits non-zero on the first run that has any
// failure, so CI can call it directly.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { readFileSync } from "node:fs";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const { curatedCategories } = await import(join(root, "src/lib/curated.js"));
const { ICONS } = await import(join(root, "src/lib/components/ui/icons.js"));

const AM_ICON = "https://raw.githubusercontent.com/Portable-Linux-Apps/Portable-Linux-Apps.github.io/main/icons/";
const AM_SCRIPT = "https://raw.githubusercontent.com/ivan-hc/AM/main/programs/x86_64/";
const AUR_INFO = "https://aur.archlinux.org/rpc/v5/info?arg[]=";

const failures = [];
const report = (ok, what, detail = "") => {
  const mark = ok ? "ok  " : "FAIL";
  console.log(`${mark}  ${what}${detail ? `  (${detail})` : ""}`);
};

function pacmanRepo(pkg) {
  const r = spawnSync("pacman", ["-Si", pkg], { encoding: "utf8" });
  if (r.error) return { ok: false, detail: `pacman error: ${r.error.message}` };
  const m = r.stdout.match(/^Repository\s*:\s*(\S+)/m);
  if (!m) return { ok: false, detail: "not in the official repos" };
  return { ok: true, detail: m[1] };
}

async function aurPkg(pkg) {
  const url = AUR_INFO + encodeURIComponent(pkg);
  let res;
  try {
    res = await fetch(url);
  } catch (e) {
    return { ok: false, detail: `network error (unverified): ${e.message}` };
  }
  if (!res.ok) return { ok: false, detail: `AUR RPC HTTP ${res.status}` };
  const j = await res.json();
  if (j.resultcount === 1) return { ok: true, detail: "aur" };
  return { ok: false, detail: `AUR resultcount ${j.resultcount}` };
}

async function amApp(id) {
  let res;
  try {
    res = await fetch(AM_SCRIPT + encodeURIComponent(id));
  } catch (e) {
    return { ok: false, detail: `network error (unverified): ${e.message}` };
  }
  if (res.ok) return { ok: true, detail: "am" };
  return { ok: false, detail: `no AM install script (HTTP ${res.status})` };
}

const seenCat = new Set();
const seenPkg = new Set();
let total = 0;

for (const cat of curatedCategories) {
  if (!cat.id || !cat.title || !cat.blurb || !cat.icon || !Array.isArray(cat.apps)) {
    report(false, `category ${cat.id || "(no id)"}`, "missing id/title/blurb/icon/apps");
    failures.push("category shape");
    continue;
  }
  if (seenCat.has(cat.id)) {
    report(false, `category ${cat.id}`, "duplicate category id");
    failures.push("category id");
  }
  seenCat.add(cat.id);
  if (!(cat.icon in ICONS)) {
    report(false, `category ${cat.id}`, `icon "${cat.icon}" is not in icons.js`);
    failures.push("icon");
  }

  for (const app of cat.apps) {
    total += 1;
    const label = `${cat.id} › ${app.name} (${app.pkg}, ${app.source})`;
    if (!app.name || !app.pkg || !app.desc || !app.source) {
      report(false, label, "missing name/pkg/desc/source");
      failures.push(label);
      continue;
    }
    if (!["repo", "aur", "appimage"].includes(app.source)) {
      report(false, label, `unknown source "${app.source}"`);
      failures.push(label);
      continue;
    }
    if (seenPkg.has(`${app.source}:${app.pkg}`)) {
      report(false, label, "duplicate pkg across the lists");
      failures.push(label);
    }
    seenPkg.add(`${app.source}:${app.pkg}`);

    if (app.source === "repo") {
      const r = pacmanRepo(app.pkg);
      report(r.ok, label, r.detail);
      if (!r.ok) failures.push(label);
    } else if (app.source === "aur") {
      const r = await aurPkg(app.pkg);
      report(r.ok, label, r.detail);
      if (!r.ok) failures.push(label);
    } else {
      const r = await amApp(app.pkg);
      report(r.ok, label, r.detail);
      if (!r.ok) failures.push(label);
    }

    // an app's own artwork: the AM icon set's <slug>.png must exist, or the
    // card would silently fall back to the package glyph
    if (app.icon && app.source !== "appimage") {
      let ok = false;
      let detail = "";
      try {
        const res = await fetch(`${AM_ICON}${encodeURIComponent(app.icon)}.png`, { method: "HEAD" });
        ok = res.ok;
        detail = ok ? `icon ${app.icon}.png` : `icon ${app.icon}.png is HTTP ${res.status}`;
      } catch (e) {
        detail = `icon ${app.icon}.png: ${e.message}`;
      }
      report(ok, `${label} icon`, detail);
      if (!ok) failures.push(`${label} icon`);
    }
  }
}

console.log(`\n${total} apps checked across ${curatedCategories.length} categories.`);
if (failures.length) {
  console.log(`${failures.length} failure(s):`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
} else {
  console.log("all packages verified.");
}
