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
    store.save().map_err(estr)
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
