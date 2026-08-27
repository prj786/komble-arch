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
/// of truth, the manifest is the syncable reflection.
fn mirror_manifest(app: &AppHandle) {
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
    let manifest = serde_json::json!({ "appimages": ai, "packages": pk });
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let farm = std::path::PathBuf::from(&home).join(".config/quickshell/../../bin/ewe-conf");
    let bin = if farm.exists() {
        farm
    } else {
        std::path::PathBuf::from("/usr/bin/ewe-conf")
    };
    if !bin.exists() {
        return; // pre-0.9 DE — the manifest simply is not mirrored yet
    }
    let _ = std::process::Command::new(bin)
        .args(["set", "--no-hooks", "apps.installed"])
        .arg(manifest.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
