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
/// missing, valid or not — plus `missing` (what restore would fetch) and
/// `safeMode`.
#[tauri::command]
pub async fn plugin_list() -> Result<Value, String> {
    let s = run(&["list", "--json"]).await?;
    serde_json::from_str(&s).map_err(|_| "ewe-plugin: unreadable reply".to_string())
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
    const KINDS: &[&str] = &[
        "service",
        "panel",
        "overlay",
        "menu",
        "bar-widget",
        "desktop-widget",
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
