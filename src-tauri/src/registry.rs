//! Persistent registries (tauri-plugin-store):
//! - registry.json / "appimages": installed AppImages keyed by id
//! - packages.json / "packages": pacman/AUR packages installed through Komble,
//!   keyed by package name

use serde_json::{Map, Value};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::util::estr;

const APPIMAGES: (&str, &str) = ("registry.json", "appimages");
const PKGS: (&str, &str) = ("packages.json", "packages");

/// Where a package came from, as the manifest records it: `repo`, `aur` or
/// `first-party`. Komble used to write the built package's FILE PATH here
/// (`…/aur/<pkg>/<pkg>-1.2-1-x86_64.pkg.tar.zst`), and the restore view
/// decides "aur" by `source == "aur"` — so every restored AUR app was handed
/// to `pacman -S`, which cannot find it (first bare-metal restore,
/// 2026-09-02). Paths are still understood, so old entries migrate on the
/// next mirror.
pub(crate) fn classify_source(source: &str) -> &'static str {
    match source {
        "aur" => "aur",
        "first-party" => "first-party",
        s if s.contains("/aur/") => "aur",
        s if s.contains("/first-party/") => "first-party",
        _ => "repo",
    }
}

fn read_map(app: &AppHandle, (file, key): (&str, &str)) -> Result<Map<String, Value>, String> {
    let store = app.store(file).map_err(estr)?;
    Ok(store
        .get(key)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default())
}

fn write_map(
    app: &AppHandle,
    (file, key): (&str, &str),
    map: Map<String, Value>,
) -> Result<(), String> {
    let store = app.store(file).map_err(estr)?;
    store.set(key, Value::Object(map));
    store.save().map_err(estr)?;
    mirror_manifest(app);
    Ok(())
}

/// RFC-001 Phase 6 / RFC-005: mirror what Komble manages into the one file
/// (`apps.installed` in ~/.config/ewe/ewe.conf) so a restored machine knows
/// what to reinstall. The registry stores stay the source of truth, the
/// manifest is the syncable reflection — UNIONED with what the manifest
/// already carries. On a fresh machine the restored manifest lists 15 apps
/// while Komble's local stores know none; a wholesale rewrite here used to
/// truncate the list on the FIRST restored install. Entries Komble manages
/// locally win (fresher versions); foreign entries survive until
/// `manifest_forget` removes them on an explicit uninstall.
///
/// Written synchronously: an install returns only after the file carries it,
/// so `ewe-conf get apps.installed` right after the action shows it (the old
/// fire-and-forget thread raced exactly that check).
pub(crate) fn mirror_manifest(app: &AppHandle) {
    let Some(bin) = manifest_bin() else { return };
    // A manifest that could not be read is not an empty one. Writing the
    // union with "nothing" would drop every app this machine has not
    // installed again yet, and the next sync would take that to the account.
    let Some(existing) = read_manifest(&bin) else {
        eprintln!("komble: apps.installed could not be read; not mirroring this time");
        return;
    };
    write_manifest(&bin, manifest_from(app, &existing).to_string());
}

/// The manifest as Komble would write it now: local registry entries
/// (sources normalised to repo | aur | first-party, paths moved to `file`)
/// unioned with the file's foreign entries. Pure — nothing is written.
pub(crate) fn build_manifest(app: &AppHandle) -> Value {
    let existing = manifest_bin()
        .and_then(|bin| read_manifest(&bin))
        .unwrap_or_default();
    manifest_from(app, &existing)
}

fn trim(v: &Value, keys: &[&str]) -> Value {
    let mut m = Map::new();
    if let Some(o) = v.as_object() {
        for k in keys {
            if let Some(x) = o.get(*k) {
                m.insert((*k).into(), x.clone());
            }
        }
    }
    Value::Object(m)
}

/// An AppImage as the manifest lists it: id, name, version and `github`, the
/// release page For you installs it from again. The registry calls that
/// field `repo` (appimage.rs). Until 0.26.1 the manifest copied a `github`
/// the registry never had, so no AppImage in a backup could be installed
/// again: For you sent every one of them to Discover.
fn appimage_entry(v: &Value) -> Value {
    let mut e = trim(v, &["id", "name", "version"]);
    let source = ["repo", "github"]
        .iter()
        .filter_map(|k| v.get(*k).and_then(|s| s.as_str()))
        .find(|s| !s.is_empty());
    if let Some(source) = source {
        e["github"] = Value::String(source.to_string());
    }
    e
}

fn manifest_from(app: &AppHandle, existing: &Map<String, Value>) -> Value {
    let ai: Vec<Value> = read_map(app, APPIMAGES)
        .unwrap_or_default()
        .values()
        .map(appimage_entry)
        .collect();
    let pk: Vec<Value> = read_map(app, PKGS)
        .unwrap_or_default()
        .values()
        .map(|v| {
            let mut e = trim(v, &["package", "version", "source", "file"]);
            // migrate: a path in `source` becomes its kind, the path moves to `file`
            let src = e
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            if src.starts_with('/') && e.get("file").is_none() {
                e["file"] = Value::String(src.clone());
            }
            e["source"] = Value::String(classify_source(&src).to_string());
            e
        })
        .collect();
    // union with the existing manifest: local entries win per key, foreign
    // (not-yet-restored) entries survive
    let ai = union_by(ai, existing.get("appimages"), "id");
    let pk = union_by(pk, existing.get("packages"), "package");
    serde_json::json!({ "appimages": ai, "packages": pk })
}

fn manifest_bin() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let farm = std::path::PathBuf::from(&home).join(".config/quickshell/../../bin/ewe-conf");
    if farm.exists() {
        return Some(farm);
    }
    let usr = std::path::PathBuf::from("/usr/bin/ewe-conf");
    if usr.exists() {
        return Some(usr); // pre-0.9 DE otherwise — the manifest simply is not mirrored
    }
    None
}

/// `apps.installed` as the file has it. None when it could not be read at
/// all: ewe-conf did not run, or answered something else than the manifest
/// or "there is none yet".
fn read_manifest(bin: &std::path::Path) -> Option<Map<String, Value>> {
    let out = std::process::Command::new(bin)
        .args(["get", "apps.installed"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    parse_manifest(&out.stdout)
}

/// ewe-conf's answer to `get apps.installed`: the manifest, or
/// `{"error": "no-manifest"}` on a machine that has none yet (an empty one).
/// Any other error, or no JSON object, is not a manifest.
fn parse_manifest(stdout: &[u8]) -> Option<Map<String, Value>> {
    let v: Value = serde_json::from_slice(stdout).ok()?;
    let m = v.as_object()?;
    match m.get("error").and_then(|e| e.as_str()) {
        None => Some(m.clone()),
        Some("no-manifest") | Some("no-key") => Some(Map::new()),
        Some(_) => None,
    }
}

/// Run to completion (ewe-conf is a Python start-up, ~100 ms) and LOGGED: a
/// spawn-and-forget here is how a machine's app list once silently failed to
/// reach the file — the failure was invisible.
fn write_manifest(bin: &std::path::Path, manifest: String) {
    let out = std::process::Command::new(bin)
        .args(["set", "--no-hooks", "apps.installed"])
        .arg(manifest)
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => eprintln!(
            "komble: ewe-conf set apps.installed failed ({}): {}",
            o.status,
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => eprintln!("komble: could not run {}: {e}", bin.display()),
    }
}

fn union_by(local: Vec<Value>, existing: Option<&Value>, key: &str) -> Vec<Value> {
    let mut out = local;
    let have: std::collections::HashSet<String> = out
        .iter()
        .filter_map(|v| v.get(key).and_then(|k| k.as_str()).map(String::from))
        .collect();
    if let Some(arr) = existing.and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(k) = v.get(key).and_then(|k| k.as_str()) {
                if !have.contains(k) {
                    out.push(v.clone());
                }
            }
        }
    }
    out
}

/// An explicit uninstall is the ONE thing allowed to shrink the manifest —
/// mirror_manifest's union would otherwise resurrect the entry forever.
pub(crate) fn manifest_forget(kind: &str, name: &str) {
    let Some(bin) = manifest_bin() else { return };
    let Some(mut m) = read_manifest(&bin) else {
        return;
    };
    let key = if kind == "appimages" { "id" } else { "package" };
    if let Some(arr) = m.get_mut(kind).and_then(|v| v.as_array_mut()) {
        arr.retain(|v| v.get(key).and_then(|k| k.as_str()) != Some(name));
    }
    write_manifest(&bin, Value::Object(m).to_string());
}

pub fn appimages(app: &AppHandle) -> Result<Vec<Value>, String> {
    Ok(read_map(app, APPIMAGES)?.values().cloned().collect())
}

pub fn get_appimage(app: &AppHandle, id: &str) -> Result<Option<Value>, String> {
    Ok(read_map(app, APPIMAGES)?.get(id).cloned())
}

pub fn upsert_appimage(app: &AppHandle, id: &str, entry: Value) -> Result<(), String> {
    let mut m = read_map(app, APPIMAGES)?;
    m.insert(id.to_string(), entry);
    write_map(app, APPIMAGES, m)
}

pub fn remove_appimage(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut m = read_map(app, APPIMAGES)?;
    m.remove(id);
    write_map(app, APPIMAGES, m)
}

pub fn packages(app: &AppHandle) -> Result<Vec<Value>, String> {
    Ok(read_map(app, PKGS)?.values().cloned().collect())
}

pub fn upsert_package(app: &AppHandle, package: &str, entry: Value) -> Result<(), String> {
    let mut m = read_map(app, PKGS)?;
    m.insert(package.to_string(), entry);
    write_map(app, PKGS, m)
}

pub fn remove_package(app: &AppHandle, package: &str) -> Result<(), String> {
    let mut m = read_map(app, PKGS)?;
    m.remove(package);
    write_map(app, PKGS, m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_appimage_takes_its_release_page_from_the_registrys_repo() {
        let registry = json!({
            "id": "obsidian", "name": "Obsidian", "version": "1.9.2",
            "path": "/home/a/.local/share/appimages/obsidian.AppImage",
            "repo": "https://github.com/obsidianmd/obsidian-releases",
        });
        assert_eq!(
            appimage_entry(&registry),
            json!({
                "id": "obsidian", "name": "Obsidian", "version": "1.9.2",
                "github": "https://github.com/obsidianmd/obsidian-releases",
            })
        );
    }

    #[test]
    fn an_appimage_from_a_local_file_has_no_release_page() {
        let registry = json!({ "id": "tool", "name": "Tool", "version": "1", "repo": null });
        assert_eq!(
            appimage_entry(&registry),
            json!({ "id": "tool", "name": "Tool", "version": "1" })
        );
    }

    #[test]
    fn no_manifest_yet_is_an_empty_one() {
        let none = br#"{"error": "no-manifest", "key": "apps.installed"}"#;
        assert_eq!(parse_manifest(none), Some(Map::new()));
    }

    #[test]
    fn a_manifest_is_read_as_it_is() {
        let m = parse_manifest(br#"{"appimages": [], "packages": [{"package": "gimp"}]}"#);
        assert_eq!(m.unwrap()["packages"][0]["package"], "gimp");
    }

    #[test]
    fn an_answer_that_is_not_a_manifest_is_not_an_empty_one() {
        assert_eq!(parse_manifest(b""), None);
        assert_eq!(parse_manifest(b"Traceback (most recent call last):"), None);
        assert_eq!(parse_manifest(br#"{"error": "bad-key"}"#), None);
        assert_eq!(parse_manifest(b"[]"), None);
    }
}
