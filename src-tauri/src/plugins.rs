//! Shell plugins, through `ewe-plugin` (ewe 0.14+). The CLI is the single
//! implementation — it clones, validates the manifest, writes [plugins] in
//! ewe.conf through ewe-conf and restarts the shell; this file only runs it
//! and hands its words to the UI. Nothing about a plugin is decided here.
//!
//! Komble survives the shell restart a toggle causes: ewe.service has
//! KillMode=process, so only quickshell itself goes down for a second.
use serde_json::Value;

fn plugin_bin() -> Option<std::path::PathBuf> {
    crate::de::ewe_bin("ewe-plugin")
}

/// Run one verb. Failures come back as the tool's own line (its `die()`
/// prints "ewe-plugin: <why>" on stderr), prefix stripped, for the toast.
async fn run(args: &[&str]) -> Result<String, String> {
    let Some(bin) = plugin_bin() else {
        return Err("ewe-plugin is not installed — the desktop needs ewe 0.14 or newer".into());
    };
    let out = tokio::process::Command::new(bin)
        .args(args)
        .output()
        .await
        .map_err(crate::util::estr)?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let why = if stderr.is_empty() { stdout } else { stderr };
        Err(why.trim_start_matches("ewe-plugin: ").to_string())
    }
}

fn check_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    {
        return Err("bad plugin id".into());
    }
    Ok(())
}

/// `list --json`: every plugin known here or to the file — installed,
/// missing, valid or not — plus `missing` (what restore would fetch),
/// `safeMode`, and from ewe 0.25 the add-ons catalog: `available` (every
/// add-on the ewe payload ships, normalised — see [`Addon`]) and `removed`
/// (bundled ids the person removed; they stay in `available`, uninstalled).
/// An older ewe has neither key, and the UI hides the catalog.
#[tauri::command]
pub async fn plugin_list() -> Result<Value, String> {
    let s = run(&["list", "--json"]).await?;
    let v: Value =
        serde_json::from_str(&s).map_err(|_| "ewe-plugin: unreadable reply".to_string())?;
    Ok(normalize_list(v))
}

/// What an add-on needs on this machine: pacman packages and commands on
/// PATH. `requires` is what the manifest declares, `missing` what is absent.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Needs {
    pub packages: Vec<String>,
    pub commands: Vec<String>,
}

impl Needs {
    fn from_value(v: Option<&Value>) -> Needs {
        Needs {
            packages: strings_of(v.and_then(|n| n.get("packages"))),
            commands: strings_of(v.and_then(|n| n.get("commands"))),
        }
    }
}

/// One add-on of the ewe payload, as `list --json` → `available[]` reports it
/// (ewe 0.25+). The tool's JSON is the contract; every field but `id` is
/// optional here so a field the tool does not send yet never hides the whole
/// catalog. `icon` is a Theme icon NAME (`icMusic`); the UI maps it onto the
/// shared Lucide table and falls back to the puzzle piece.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Addon {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub version: String,
    pub kinds: Vec<String>,
    pub installed: bool,
    pub enabled: bool,
    pub requires: Needs,
    pub missing: Needs,
}

fn string_of(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn strings_of(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

impl Addon {
    /// `None` when there is no usable id — a catalog entry nothing could act
    /// on. `fallback` is the `plugins[]` entry of the same id, for the state
    /// flags when the catalog entry leaves them out.
    fn from_value(v: &Value, fallback: Option<&Value>) -> Option<Addon> {
        let id = string_of(v.get("id"));
        if check_id(&id).is_err() {
            return None;
        }
        let flag = |key: &str| -> bool {
            v.get(key)
                .or_else(|| fallback.and_then(|f| f.get(key)))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        let name = string_of(v.get("name"));
        Some(Addon {
            name: if name.is_empty() { id.clone() } else { name },
            description: string_of(v.get("description")),
            icon: string_of(v.get("icon")),
            category: string_of(v.get("category")),
            version: string_of(v.get("version")),
            kinds: strings_of(v.get("kinds")),
            installed: flag("installed"),
            enabled: flag("enabled"),
            requires: Needs::from_value(v.get("requires")),
            missing: Needs::from_value(v.get("missing")),
            id,
        })
    }
}

/// Normalise the catalog half of a `list --json` reply in place: `available`
/// becomes a list of well-formed [`Addon`]s (dropping entries without an id),
/// `removed` a list of ids. Both are left ABSENT when the tool did not send
/// them — that is how the UI tells an older ewe from an empty catalog.
fn normalize_list(mut v: Value) -> Value {
    let plugins: Vec<Value> = v
        .get("plugins")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(avail) = v.get("available").and_then(Value::as_array).cloned() {
        let addons: Vec<Addon> = avail
            .iter()
            .filter_map(|a| {
                let fallback = plugins.iter().find(|p| {
                    p.get("id") == a.get("id") && p.get("installed") == Some(&Value::Bool(true))
                });
                Addon::from_value(a, fallback)
            })
            .collect();
        v["available"] = serde_json::to_value(addons).unwrap_or(Value::Array(vec![]));
    }
    if v.get("removed").is_some() {
        v["removed"] = Value::Array(
            strings_of(v.get("removed"))
                .into_iter()
                .map(Value::String)
                .collect(),
        );
    }
    v
}

/// The tool's JSON result of a state verb: `{"ok": false, "error": …}` is a
/// failure even with exit 0 (every ewe CLI prints JSON and exits 0 by the
/// house rules; the older verbs still `die()` with a non-zero exit, which
/// `run` already turns into an Err). Non-JSON output is wrapped as `raw`.
fn result_value(out: &str) -> Result<Value, String> {
    let Ok(v) = serde_json::from_str::<Value>(out) else {
        return Ok(serde_json::json!({ "ok": true, "raw": out }));
    };
    if v.get("ok") == Some(&Value::Bool(false)) {
        let why = ["error", "message", "reason"]
            .iter()
            .find_map(|k| v.get(k).and_then(Value::as_str))
            .unwrap_or("ewe-plugin refused");
        return Err(why.to_string());
    }
    Ok(v)
}

/// Install an add-on from the ewe payload (`ewe-plugin install`, ewe 0.25+):
/// copied from the payload with source `bundled`, enabled, keybinds
/// regenerated, the shell restarted. When the catalog says packages are
/// missing they are installed first, through the same privileged pacman path
/// as every other repo install (one polkit prompt) — the catalog decides what
/// is missing, never the webview. Nothing is recorded in the apps manifest:
/// an add-on's dependencies are not apps the person chose.
#[tauri::command]
pub async fn plugin_install(id: String) -> Result<Value, String> {
    check_id(&id)?;
    let list = plugin_list().await?;
    let Some(available) = list.get("available").and_then(Value::as_array) else {
        return Err("add-ons need ewe 0.25 or newer".into());
    };
    let Some(addon) = available
        .iter()
        .find(|a| a.get("id").and_then(Value::as_str) == Some(&id))
    else {
        return Err(format!("{id} is not an add-on of this ewe"));
    };
    let pkgs = strings_of(addon.pointer("/missing/packages"));
    if !pkgs.is_empty() {
        crate::pacman::install_packages_named(&pkgs).await?;
    }
    let out = run(&["install", &id]).await?;
    result_value(&out)
}

/// Clone from a git URL. The UI never installs a local directory — that is a
/// developer's path and belongs to the terminal.
#[tauri::command]
pub async fn plugin_add(url: String, enable: bool) -> Result<String, String> {
    let u = url.trim();
    if !(u.starts_with("https://") || u.starts_with("ssh://") || u.starts_with("git@"))
        || u.len() > 512
    {
        return Err("a git URL, please — https://… or git@…".into());
    }
    let mut args = vec!["add", u, "--yes"];
    if enable {
        args.push("--enable");
    }
    run(&args).await
}

#[tauri::command]
pub async fn plugin_set_enabled(id: String, on: bool) -> Result<String, String> {
    check_id(&id)?;
    run(&[if on { "enable" } else { "disable" }, &id]).await
}

#[tauri::command]
pub async fn plugin_update(id: Option<String>) -> Result<String, String> {
    match id {
        Some(id) => {
            check_id(&id)?;
            run(&["update", &id, "--yes"]).await
        }
        None => run(&["update", "--yes"]).await,
    }
}

#[tauri::command]
pub async fn plugin_remove(id: String) -> Result<String, String> {
    check_id(&id)?;
    run(&["remove", &id, "--yes"]).await
}

/// Fetch every plugin the file knows that is not installed here — the
/// plugin half of "For you". Never automatic: the UI asks first.
#[tauri::command]
pub async fn plugin_restore() -> Result<String, String> {
    run(&["restore", "--yes"]).await
}

/// `ewe-plugin create`: a new plugin repository (manifest, one working QML
/// per kind, README, licence, git init) in `dir`. The UI's "New plugin…".
#[tauri::command]
pub async fn plugin_create(
    id: String,
    name: String,
    kinds: Vec<String>,
    dir: String,
) -> Result<String, String> {
    check_id(&id)?;
    if !id.contains('.') {
        return Err("id must be <namespace>.<name>".into());
    }
    // every kind the ewe plugin API knows (API 3 added the last four)
    const KINDS: &[&str] = &[
        "service",
        "panel",
        "overlay",
        "menu",
        "bar-widget",
        "desktop-widget",
        "quick-tile",
        "quick-page",
        "bar-status",
        "dock-item",
    ];
    if kinds.is_empty() || kinds.iter().any(|k| !KINDS.contains(&k.as_str())) {
        return Err("pick at least one kind".into());
    }
    let dir = dir.trim();
    if dir.is_empty() || !dir.starts_with('/') || dir.len() > 512 {
        return Err("an absolute folder to create it in, please".into());
    }
    let name = if name.trim().is_empty() {
        id.clone()
    } else {
        name.trim().to_string()
    };
    let kinds = kinds.join(",");
    let dest = format!("{}/{}", dir.trim_end_matches('/'), id);
    run(&[
        "create", &id, "--name", &name, "--kinds", &kinds, "--dir", &dest,
    ])
    .await?;
    Ok(dest)
}

/// A declared setting's value (typed by the plugin's manifest; ewe-plugin
/// refuses anything that does not fit). Live — the shell re-reads.
#[tauri::command]
pub async fn plugin_set(id: String, key: String, value: String) -> Result<String, String> {
    check_id(&id)?;
    if key.is_empty()
        || key.len() > 32
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("bad setting key".into());
    }
    if value.len() > 512 {
        return Err("value too long".into());
    }
    run(&["set", &id, &key, &value]).await
}

/// A desktop widget's layer (desktop | top = sticky) or visibility. Live.
#[tauri::command]
pub async fn plugin_place(
    id: String,
    layer: Option<String>,
    visible: Option<bool>,
) -> Result<String, String> {
    check_id(&id)?;
    let mut args: Vec<String> = vec!["place".into(), id];
    if let Some(l) = layer {
        if l != "desktop" && l != "top" {
            return Err("layer is desktop or top".into());
        }
        args.push("--layer".into());
        args.push(l);
    }
    if let Some(v) = visible {
        args.push("--visible".into());
        args.push(if v { "on".into() } else { "off".into() });
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_entries_are_normalised_with_defaults() {
        let v = normalize_list(json!({
            "plugins": [
                { "id": "ewe.clipboard", "installed": true, "enabled": true, "bundled": true }
            ],
            "available": [
                { "id": "ewe.media", "name": "Music", "icon": "icMusic", "category": "Media",
                  "version": "1.0.0", "kinds": ["panel", "dock-item"], "installed": false,
                  "requires": { "packages": ["playerctl"] },
                  "missing": { "packages": ["playerctl"], "commands": [] } },
                // state flags left out: taken from the installed plugins[] entry
                { "id": "ewe.clipboard", "name": "Clipboard" },
                // no requires/missing at all, null icon, blank name → id
                { "id": "ewe.vpn", "name": " ", "icon": null },
                // nothing to act on
                { "name": "no id" },
                { "id": "Bad Id!" }
            ],
            "removed": ["ewe.screenshot", 7, " "]
        }));
        let avail = v["available"].as_array().unwrap();
        assert_eq!(avail.len(), 3);
        assert_eq!(avail[0]["id"], "ewe.media");
        assert_eq!(avail[0]["requires"]["packages"], json!(["playerctl"]));
        assert_eq!(avail[0]["requires"]["commands"], json!([]));
        assert_eq!(avail[0]["missing"]["packages"], json!(["playerctl"]));
        assert_eq!(avail[0]["installed"], false);
        assert_eq!(avail[1]["installed"], true);
        assert_eq!(avail[1]["enabled"], true);
        assert_eq!(avail[2]["name"], "ewe.vpn");
        assert_eq!(avail[2]["icon"], "");
        assert_eq!(avail[2]["missing"]["packages"], json!([]));
        assert_eq!(avail[2]["installed"], false);
        assert_eq!(v["removed"], json!(["ewe.screenshot"]));
    }

    #[test]
    fn an_older_ewe_has_no_catalog_keys() {
        let v = normalize_list(json!({ "plugins": [], "missing": [], "safeMode": false }));
        assert!(v.get("available").is_none());
        assert!(v.get("removed").is_none());
    }

    #[test]
    fn install_results() {
        assert!(result_value("{\"ok\": true, \"id\": \"ewe.media\"}").is_ok());
        assert_eq!(
            result_value("installed ewe.media").unwrap()["raw"],
            "installed ewe.media"
        );
        assert_eq!(
            result_value("{\"ok\": false, \"error\": \"not in the payload\"}").unwrap_err(),
            "not in the payload"
        );
    }
}

/// Arrange mode on the desktop (drag widgets, sticky, hide) — the shell's
/// `widgets arrange` IPC verb, same as Super+Shift+W.
#[tauri::command]
pub async fn plugin_arrange() -> Result<(), String> {
    let out = tokio::process::Command::new("qs")
        .args(["ipc", "call", "widgets", "arrange"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("the desktop shell did not answer — is ewe 0.20+ running?".into());
    }
    Ok(())
}
