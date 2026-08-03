//! hypr-shell integration: Komble is a first-class citizen of the DE, so it
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
/// (Komble outside hypr-shell) → null; the frontend keeps its defaults.
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
async fn shell_pid() -> Option<String> {
    let out = run_out(
        "systemctl",
        &["--user", "show", "-p", "MainPID", "--value", "hypr-shell.service"],
    )
    .await
    .ok()?;
    let pid = out.trim().to_string();
    if pid.is_empty() || pid == "0" {
        None
    } else {
        Some(pid)
    }
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
    tokio::spawn(async {
        let _ = qs_ipc("syncSoon".into()).await;
    });
}

/// The package lists the shell cached from the cloud bundle
/// (google-restore-packages.json, written by `qs ipc call google
/// fetchPackages`), diffed against what is installed — APPS ONLY.
#[tauri::command]
pub async fn backup_packages() -> Result<Value, String> {
    let p = home().join(".config/quickshell/google-restore-packages.json");
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => return Ok(Value::Null),
    };
    let j: Value = serde_json::from_str(&text).map_err(estr)?;

    let installed: std::collections::HashSet<String> = run_out("pacman", &["-Qq"])
        .await
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let list = |k: &str| -> Vec<String> {
        j[k].as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let explicit = list("explicit");
    let foreign = list("foreign");
    let desktop = list("desktopApps");

    // Newer bundles carry the app subset (packages owning a .desktop file).
    // Older ones don't — fall back to a system-package exclusion heuristic so
    // a kernel or firmware line never shows up as an "app to restore".
    let is_appish = |n: &str| {
        if !desktop.is_empty() {
            return desktop.iter().any(|d| d == n);
        }
        let sys = n.starts_with("linux")
            || n.starts_with("lib")
            || n.ends_with("-firmware")
            || n.ends_with("-ucode")
            || n.ends_with("-headers")
            || n.ends_with("-dkms")
            || matches!(n, "base" | "base-devel" | "grub" | "efibootmgr" | "mkinitcpio" | "sudo");
        !sys
    };

    let foreign_set: std::collections::HashSet<&String> = foreign.iter().collect();
    let mut apps: Vec<Value> = explicit
        .iter()
        .chain(foreign.iter())
        .filter(|n| is_appish(n))
        .map(|n| {
            json!({
                "name": n,
                "installed": installed.contains(n.as_str()),
                "aur": foreign_set.contains(n),
            })
        })
        .collect();
    apps.sort_by(|a, b| {
        (
            a["installed"].as_bool().unwrap_or(false),
            a["name"].as_str().unwrap_or("").to_string(),
        )
            .cmp(&(
                b["installed"].as_bool().unwrap_or(false),
                b["name"].as_str().unwrap_or("").to_string(),
            ))
    });

    Ok(json!({
        "ok": j["ok"].as_bool().unwrap_or(false),
        "fetchedAt": j["fetchedAt"],
        "updatedAt": j["updatedAt"],
        "device": j["device"],
        "apps": apps,
    }))
}
