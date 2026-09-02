//! ewe integration: Komble is a first-class citizen of the DE, so it
//! reads the desktop's accent/look and talks to the shell's Google account
//! over `qs ipc` — a fixed verb allowlist, tokens never cross the boundary.

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::{json, Value};
use tokio::process::Command;

use crate::util::estr;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

/// The DE's user-theme.json — accent + look + app colour scheme. Missing file
/// (Komble outside ewe) → null; the frontend keeps its defaults.
#[tauri::command]
pub async fn de_prefs() -> Result<Value, String> {
    let p = home().join(".config/quickshell/user-theme.json");
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => return Ok(Value::Null),
    };
    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if j.is_null() {
        return Ok(Value::Null);
    }
    let theme_name = j["themeName"].as_str().unwrap_or("flock").to_string();
    let accent = j["accent"].as_str().unwrap_or("#0a84ff").to_string();
    Ok(json!({
        "accent": accent,
        "themeName": theme_name,
        "colorScheme": j["colorScheme"].as_str().unwrap_or("dark"),
    }))
}

async fn run_out(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(estr)?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The DE shell's qs pid — targeting it explicitly makes `qs ipc` unambiguous
/// even when another qs instance exists (e.g. a nested test session).
/// The user unit is `ewe.service` since the rename; `hypr-shell.service` is
/// probed second for machines still on a pre-rename install.
async fn shell_pid() -> Option<String> {
    for unit in ["ewe.service", "hypr-shell.service"] {
        if let Ok(out) = run_out(
            "systemctl",
            &["--user", "show", "-p", "MainPID", "--value", unit],
        )
        .await
        {
            let pid = out.trim().to_string();
            if !pid.is_empty() && pid != "0" {
                return Some(pid);
            }
        }
    }
    None
}

/// Shell IPC, allowlisted: exactly the Google-account verbs "For you" needs.
#[tauri::command]
pub async fn qs_ipc(func: String) -> Result<String, String> {
    if !["status", "syncNow", "syncSoon", "fetchPackages", "signIn"].contains(&func.as_str()) {
        return Err(format!("ipc not allowed: google {func}"));
    }
    if let Some(pid) = shell_pid().await {
        return run_out("qs", &["ipc", "--pid", &pid, "call", "google", &func]).await;
    }
    run_out("qs", &["ipc", "call", "google", &func]).await
}

/// Fire-and-forget "the app list changed": the shell debounces the pokes and
/// pushes the sync bundle when things go quiet. Installs never wait on it.
pub fn poke_sync() {
    tauri::async_runtime::spawn(async {
        let _ = qs_ipc("syncSoon".into()).await;
    });
}

/// Fire-and-forget "re-count pending updates": the bar's Komble indicator
/// re-runs its checkupdates/paru probe. Poked after every transaction so the
/// glyph flips (spinner while pacman holds the lock, count/check after).
/// Raise/clear the bar indicator's "an upgrade is running" state. AUR builds
/// hold no pacman lock while makepkg runs, so without this the bar would show
/// the spinner only for the final `pacman -U` seconds of a long upgrade. The
/// shell auto-clears it after 15 min without a re-assert (crash safety).
///
/// Awaited, not spawned: the repo phase's `false` and the AUR phase's `true`
/// follow each other within milliseconds, and two spawned pokes can land out
/// of order — turning the glyph OFF for the whole build. In-order pokes cost
/// ~30 ms each, which is nothing against an upgrade.
pub async fn poke_working_now(on: bool) {
    let mut args: Vec<String> = vec!["ipc".into()];
    if let Some(pid) = shell_pid().await {
        args.push("--pid".into());
        args.push(pid);
    }
    args.extend(["call", "updates", "working"].map(String::from));
    args.push(if on { "true".into() } else { "false".into() });
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let _ = run_out("qs", &refs).await;
}

/// Fire-and-forget variant for mid-run re-asserts, where ordering can't matter.
/// tauri's spawner, NOT tokio::spawn: sync #[tauri::command]s run on the main
/// thread with no reactor, where tokio::spawn PANICS — and with panic=abort
/// that killed the whole app the moment the Updates view poked the shell
/// (SIGABRT ×4 on 2026-09-01).
pub fn poke_working(on: bool) {
    // tauri's spawner, NOT tokio::spawn: the sync command path runs on the
    // main thread with no reactor, where tokio::spawn PANICS — and with
    // panic=abort that killed the whole app the moment the Updates view
    // poked the shell (SIGABRT, 2026-09-01).
    tauri::async_runtime::spawn(poke_working_now(on));
}

/// Frontend-triggered poke: the Updates view fires this after its own count
/// lands, so the bar re-probes at the same moment and the two numbers move
/// together instead of up to an hour apart.
#[tauri::command]
pub fn poke_shell_updates() {
    poke_updates();
}

pub fn poke_updates() {
    tauri::async_runtime::spawn(async {
        let mut args: Vec<String> = vec!["ipc".into()];
        if let Some(pid) = shell_pid().await {
            args.push("--pid".into());
            args.push(pid);
        }
        args.extend(["call", "updates", "refresh"].map(String::from));
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let _ = run_out("qs", &refs).await;
    });
}

// ── RFC-002 step 4: the restore surface ─────────────────────────────────────
// A synced ewe.conf carries [apps.installed] — everything Komble managed on
// the machine that wrote it. After a pull on a fresh machine, this reads the
// manifest THROUGH ewe-conf (never the file directly) and reports what is
// missing here, so the UI can offer the reinstall list. Explicit confirm
// stays with the user: a restored file must never silently install software.

fn ewe_conf_bin() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let farm = std::path::PathBuf::from(&home).join(".config/quickshell/../../bin/ewe-conf");
    if farm.exists() {
        return Some(farm);
    }
    let usr = std::path::PathBuf::from("/usr/bin/ewe-conf");
    usr.exists().then_some(usr)
}

#[tauri::command]
pub async fn restore_manifest(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let Some(bin) = ewe_conf_bin() else {
        return Ok(serde_json::json!({"available": false, "reason": "no-ewe-conf"}));
    };
    let out = tokio::process::Command::new(bin)
        .args(["get", "apps.installed"])
        .output()
        .await
        .map_err(crate::util::estr)?;
    if !out.status.success() {
        // newer ewe-conf prints {"error": "..."} on stderr for a missing key;
        // older ones just exit 1 — both mean the file carries no app list
        let reason = serde_json::from_slice::<serde_json::Value>(&out.stderr)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "no-manifest".into());
        return Ok(serde_json::json!({"available": false, "reason": reason}));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(crate::util::estr)?;

    // every manifest entry, flagged with whether it exists HERE, and with a
    // NORMALISED source: older Komble versions wrote the built package's file
    // path into `source`, so a backup may say `/…/aur/foo/foo-1-1.pkg.tar.zst`
    // where it means "aur". First-party apps (Komble, ewe-settings) come with
    // the desktop and are never "missing" — they are skipped and counted.
    let mut pkgs = Vec::new();
    let mut skipped = 0;
    for p in manifest
        .get("packages")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![])
    {
        let Some(name) = p.get("package").and_then(|v| v.as_str()) else {
            continue;
        };
        let raw = p.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let mut source = crate::registry::classify_source(raw);
        if source == "first-party" {
            skipped += 1;
            continue;
        }
        let installed = crate::pacman::installed_version(name).await.is_some();
        // "repo" is only trusted once the repos actually know the name — a
        // hand-installed file or an old path-shaped entry may really be AUR
        if source == "repo" && !installed && !repo_knows(name).await {
            source = "aur";
        }
        let mut e = p.clone();
        e["source"] = serde_json::json!(source);
        e["installed"] = serde_json::json!(installed);
        pkgs.push(e);
    }
    let here: std::collections::HashSet<String> = crate::registry::appimages(&app)
        .ok()
        .map(|v| {
            v.iter()
                .filter_map(|a| a.get("id").and_then(|x| x.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let appimages: Vec<serde_json::Value> = manifest
        .get("appimages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    let mut e = a.clone();
                    let id = a.get("id").and_then(|x| x.as_str()).unwrap_or("");
                    e["installed"] = serde_json::json!(here.contains(id));
                    e
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(serde_json::json!({
        "available": true,
        "packages": pkgs,
        "appimages": appimages,
        "skipped": skipped,
    }))
}

/// `pacman -Si` succeeds only for a package the sync databases carry — the
/// cheapest honest "is this a repo package" there is (no `-Sy`, no network).
async fn repo_knows(name: &str) -> bool {
    Command::new("pacman")
        .args(["-Si", "--", name])
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// push/pull the one file via ewe-conf (RFC-002 sync verbs).
#[tauri::command]
pub async fn conf_sync(direction: String) -> Result<serde_json::Value, String> {
    if direction != "push" && direction != "pull" {
        return Err("direction must be push or pull".into());
    }
    let Some(bin) = ewe_conf_bin() else {
        return Err("ewe-conf not installed".into());
    };
    let out = tokio::process::Command::new(&bin)
        .arg(&direction)
        .output()
        .await
        .map_err(crate::util::estr)?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(crate::util::estr)?;
    // A pull that leaves ewe.conf = remote while every runtime file = local
    // is half a restore. Regenerate the artifacts WITH hooks: `apply` itself
    // re-themes, pokes the shell (`settings reload`) and reloads Hyprland, so
    // gaps and the window border follow the restored file at once instead of
    // waiting for a relogin (the old `--no-hooks` + shell-only poke never
    // touched Hyprland).
    if direction == "pull" && v.get("ok").and_then(|b| b.as_bool()) == Some(true) {
        let _ = tokio::process::Command::new(&bin)
            .arg("apply")
            .output()
            .await;
    }
    Ok(v)
}

/// Cloud-copy facts WITHOUT touching anything — the UI decides, the user
/// clicks. (`conf_sync("pull")` used to double as the probe, silently
/// overwriting the local file the moment the pane opened.)
#[tauri::command]
pub async fn conf_sync_status() -> Result<serde_json::Value, String> {
    let Some(bin) = ewe_conf_bin() else {
        return Err("ewe-conf not installed".into());
    };
    let out = tokio::process::Command::new(bin)
        .arg("sync-status")
        .output()
        .await
        .map_err(crate::util::estr)?;
    serde_json::from_slice(&out.stdout).map_err(crate::util::estr)
}
