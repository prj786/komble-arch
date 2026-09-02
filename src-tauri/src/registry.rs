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

/// RFC-001 Phase 6: mirror what Komble manages into the one file
/// (`apps.installed` in ~/.config/ewe/ewe.conf) so a restored machine knows
/// what to reinstall. Fire-and-forget: the registry stores stay the source
/// of truth, the manifest is the syncable reflection — UNIONED with what the
/// manifest already carries. On a fresh machine the pulled manifest lists 15
/// apps while Komble's local stores know none; a wholesale rewrite here used
/// to truncate the list on the FIRST restored install, and the shell's auto
/// push then destroyed the Drive backup. Entries Komble manages locally win
/// (fresher versions); foreign entries survive until `manifest_forget`
/// removes them on an explicit uninstall.
pub(crate) fn mirror_manifest(app: &AppHandle) {
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
    let ai: Vec<Value> = read_map(app, APPIMAGES)
        .unwrap_or_default()
        .values()
        .map(|v| trim(v, &["id", "name", "github", "version"]))
        .collect();
    let pk: Vec<Value> = read_map(app, PKGS)
        .unwrap_or_default()
        .values()
        .map(|v| trim(v, &["package", "version", "source"]))
        .collect();
    let Some(bin) = manifest_bin() else { return };
    // union with the existing manifest: local entries win per key, foreign
    // (not-yet-restored) entries survive
    let existing = read_manifest(&bin);
    let ai = union_by(ai, existing.get("appimages"), "id");
    let pk = union_by(pk, existing.get("packages"), "package");
    let manifest = serde_json::json!({ "appimages": ai, "packages": pk });
    write_manifest(&bin, manifest.to_string());
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

fn read_manifest(bin: &std::path::Path) -> Map<String, Value> {
    std::process::Command::new(bin)
        .args(["get", "apps.installed"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Off the caller's thread (ewe-conf is a Python start-up, ~100 ms), but run
/// to completion and LOGGED: a spawn-and-forget here is how a machine's
/// backup silently went out without its app list — the failure was invisible.
fn write_manifest(bin: &std::path::Path, manifest: String) {
    let bin = bin.to_path_buf();
    std::thread::spawn(move || {
        let out = std::process::Command::new(&bin)
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
    });
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
    let mut m = read_manifest(&bin);
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
