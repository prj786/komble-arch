//! First-party ewe apps (Komble itself, ewe-settings) and the desktop update.
//!
//! These are not repo or AUR packages: the ewe installer takes each app's
//! prebuilt .pkg.tar.zst straight from its GitHub release, so the store's
//! sync-DB index can never see them and `pacman -S` can never install them.
//! This module makes them first-class citizens anyway:
//!
//!   · discoverable — `browse_packages` injects DISCOVER entries (section
//!     "ewe") when the search query matches
//!   · installable / updatable — latest GitHub release asset → the existing
//!     pkexec `pacman -U` flow (`install_package_file`)
//!   · plus the DESKTOP itself: version + one-click update through the ewe
//!     repo's update.sh contract (exit 0 done, 10 update available, 20 needs a
//!     terminal for sudo, 30 dirty/diverged — see hypr-shell/update.sh), so
//!     nobody has to hand-download a .tar.zst ever again.

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::util::{estr, which};

/// name, github repo, one-line summary (shown as the search-result card).
pub const DISCOVER: &[(&str, &str, &str)] = &[
    (
        "komble-arch",
        "prj786/komble-arch",
        "Komble — the ewe desktop's software manager (this app)",
    ),
    (
        "ewe-settings",
        "prj786/ewe-settings",
        "Settings for the ewe desktop — displays, input, theme, animations",
    ),
];

const EWE_REPO: &str = "prj786/ewe";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

/// Where ewe is checked out / installed — the same convention ewe-settings
/// uses: explicit env override, else get.sh's ~/.local/share/ewe, else the
/// developer clone at ~/hypr-shell (pre-rename directory name, kept on disk).
fn ewe_dir() -> PathBuf {
    for var in ["EWE_REPO", "HYPR_SHELL_REPO"] {
        if let Ok(p) = std::env::var(var) {
            if !p.trim().is_empty() {
                return PathBuf::from(p);
            }
        }
    }
    let installed = home().join(".local/share/ewe");
    if installed.join("VERSION").is_file() {
        return installed;
    }
    home().join("hypr-shell")
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

/// pacman's own version comparison — always present on Arch. Falls back to a
/// plain inequality if vercmp is somehow missing (never "newer wins" games).
async fn version_newer(candidate: &str, current: &str) -> bool {
    if candidate.is_empty() || current.is_empty() {
        return false;
    }
    if which("vercmp") {
        if let Ok(out) = run_out("vercmp", &[candidate, current]).await {
            return out.trim().parse::<i32>().map(|n| n > 0).unwrap_or(false);
        }
    }
    candidate != current
}

/// tag + the machine-matching .pkg.tar.zst asset URL from a repo's latest
/// GitHub release (same selection rule as the installer: skip -debug-, take
/// this arch or -any).
async fn latest_release(
    repo: &str,
    token: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut req = crate::util::client().get(&url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let res = req.send().await.map_err(estr)?;
    if !res.status().is_success() {
        return Err(format!("{repo}: GitHub API HTTP {}", res.status()));
    }
    let j: Value = res.json().await.map_err(estr)?;
    let tag = j["tag_name"].as_str().unwrap_or_default();
    let version = tag.trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err(format!("{repo}: release has no tag"));
    }
    let arch = std::env::consts::ARCH; // x86_64 / aarch64 — matches uname -m
    let asset = j["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["browser_download_url"].as_str())
        .filter(|u| u.ends_with(".pkg.tar.zst") && !u.contains("-debug-"))
        .find(|u| u.contains(arch) || u.contains("-any.pkg"))
        .map(String::from);
    Ok((version, asset))
}

/// One row per first-party app: installed version (None = not installed),
/// latest release, and whether an installable prebuilt asset exists.
#[tauri::command]
pub async fn first_party_status(token: Option<String>) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for (pkg, repo, summary) in DISCOVER {
        // display without the pacman pkgrel: the row reads "0.9.2" like the
        // desktop's own row, not "0.9.2-1" (comparisons still use vercmp on
        // full strings elsewhere)
        let installed =
            crate::pacman::installed_version(pkg)
                .await
                .map(|v| match v.rsplit_once('-') {
                    Some((base, rel)) if rel.chars().all(|c| c.is_ascii_digit()) => {
                        base.to_string()
                    }
                    _ => v,
                });
        let (latest, asset) = match latest_release(repo, token.as_deref()).await {
            Ok(x) => x,
            Err(e) => {
                rows.push(json!({
                    "pkg": pkg, "repo": repo, "summary": summary,
                    "installed": installed, "latest": Value::Null,
                    "updateAvailable": false, "error": e,
                }));
                continue;
            }
        };
        // pacman versions carry a -relno suffix the release tag doesn't have
        let cur = installed.clone().unwrap_or_default();
        let cur_base = cur.split('-').next().unwrap_or("").to_string();
        rows.push(json!({
            "pkg": pkg, "repo": repo, "summary": summary,
            "installed": installed,
            "latest": latest,
            "hasAsset": asset.is_some(),
            "updateAvailable": !cur_base.is_empty() && version_newer(&latest, &cur_base).await,
        }));
    }
    Ok(rows)
}

/// Install (or update) a first-party app from its latest GitHub release, via
/// the same pkexec `pacman -U` flow as any local package file. Updating
/// komble-arch from inside Komble is fine on Linux: the running binary keeps
/// its inode; the new one is picked up at next launch.
#[tauri::command]
pub async fn install_first_party(
    app: AppHandle,
    pkg: String,
    token: Option<String>,
) -> Result<String, String> {
    let r = install_first_party_inner(app.clone(), pkg.clone(), token).await;
    // always clear the progress entry (stage events have no terminal marker)
    let _ = app.emit("install-progress", json!({ "id": pkg, "phase": "done" }));
    r
}

async fn install_first_party_inner(
    app: AppHandle,
    pkg: String,
    token: Option<String>,
) -> Result<String, String> {
    let (_, repo, _) = DISCOVER
        .iter()
        .find(|(p, _, _)| *p == pkg)
        .ok_or_else(|| format!("{pkg}: not a first-party app"))?;

    let _ = app.emit("install-progress", json!({ "id": pkg, "stage": "resolve" }));
    let (version, asset) = latest_release(repo, token.as_deref()).await?;
    let url = asset.ok_or_else(|| {
        format!("{pkg} {version}: the release has no prebuilt package for this machine")
    })?;

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(estr)?
        .join("first-party");
    std::fs::create_dir_all(&dir).map_err(estr)?;
    let dest = dir.join(format!("{pkg}-{version}.pkg.tar.zst"));

    let _ = app.emit(
        "install-progress",
        json!({ "id": pkg, "stage": "download" }),
    );
    let bytes = crate::util::client()
        .get(&url)
        .send()
        .await
        .map_err(estr)?
        .error_for_status()
        .map_err(estr)?
        .bytes()
        .await
        .map_err(estr)?;
    std::fs::write(&dest, &bytes).map_err(estr)?;

    let _ = app.emit("install-progress", json!({ "id": pkg, "stage": "install" }));
    crate::pacman::install_package_file(app.clone(), dest.to_string_lossy().to_string()).await
}

/// The desktop itself: where it lives, what version it is, and whether the
/// repo's update.sh says there is anything to pull. `git: false` means a
/// tarball (get.sh) install — update then means "re-run get.sh".
#[tauri::command]
pub async fn ewe_status(token: Option<String>) -> Result<Value, String> {
    let dir = ewe_dir();
    let version = std::fs::read_to_string(dir.join("VERSION"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if version.is_empty() {
        return Ok(json!({ "installed": false }));
    }
    let is_git = dir.join(".git").is_dir();

    if is_git && dir.join("update.sh").is_file() {
        // the repo's own contract: one JSON "check" event on stdout
        let out = run_out(
            "bash",
            &[
                &dir.join("update.sh").to_string_lossy(),
                "--check",
                "--json",
            ],
        )
        .await
        .unwrap_or_default();
        let check = out
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| v["event"] == "check");
        if let Some(c) = check {
            return Ok(json!({
                "installed": true, "git": true,
                "dir": dir.to_string_lossy(),
                "version": c["version"].as_str().unwrap_or(&version),
                "updateAvailable": c["updateAvailable"].as_bool().unwrap_or(false),
                "behind": c["behind"].as_i64().unwrap_or(0),
                "dirty": c["dirty"].as_bool().unwrap_or(false),
            }));
        }
        // git repo but the check failed (offline?) — report what we know
        return Ok(json!({
            "installed": true, "git": true,
            "dir": dir.to_string_lossy(), "version": version,
            "updateAvailable": false, "behind": 0, "dirty": false,
        }));
    }

    // tarball install — compare VERSION against the latest release tag
    let (latest, update) = match latest_release(EWE_REPO, token.as_deref()).await {
        Ok((tag, _)) => {
            let newer = version_newer(&tag, &version).await;
            (Some(tag), newer)
        }
        Err(_) => (None, false),
    };
    Ok(json!({
        "installed": true, "git": false,
        "dir": dir.to_string_lossy(), "version": version,
        "latest": latest, "updateAvailable": update,
        "dirty": false,
    }))
}

/// Run the desktop update IN-APP, streaming update.sh's JSON events to the
/// frontend as "ewe-update" events. Only possible when sudo works without a
/// terminal — update.sh itself enforces that (exit 20) and we translate it to
/// the "needs-terminal" error the frontend falls back on.
#[tauri::command]
pub async fn ewe_update(app: AppHandle) -> Result<String, String> {
    let dir = ewe_dir();
    if !dir.join("update.sh").is_file() {
        return Err(
            "This ewe install has no update.sh (tarball install) — use the terminal update.".into(),
        );
    }

    let mut child = Command::new("bash")
        .arg(dir.join("update.sh"))
        .arg("--json")
        .current_dir(&dir)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(estr)?;

    let mut lines = BufReader::new(child.stdout.take().expect("piped stdout")).lines();
    let mut last_error = String::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if v["event"] == "error" {
                last_error = v["message"].as_str().unwrap_or("update failed").to_string();
            }
            let _ = app.emit("ewe-update", v);
        } else {
            let _ = app.emit("ewe-update", json!({ "event": "log", "line": line }));
        }
    }

    let status = child.wait().await.map_err(estr)?;
    crate::de::poke_updates();
    match status.code() {
        Some(0) => Ok("ewe updated".into()),
        Some(20) => Err("needs-terminal".into()),
        Some(30) => Err(if last_error.is_empty() {
            "working tree has local changes or history diverged".into()
        } else {
            last_error
        }),
        _ => Err(if last_error.is_empty() {
            "update failed — see the log".into()
        } else {
            last_error
        }),
    }
}

/// Fallback when in-app updating is impossible (sudo needs a password, or the
/// install is a tarball): open the update in a real terminal and let the user
/// type their password there. Tries the DE's terminals in order.
#[tauri::command]
pub async fn ewe_update_terminal() -> Result<(), String> {
    let dir = ewe_dir();
    let inner = if dir.join(".git").is_dir() && dir.join("update.sh").is_file() {
        format!("cd '{}' && ./update.sh", dir.to_string_lossy())
    } else {
        // get.sh re-downloads the latest artefact and re-runs the installer
        "bash <(curl -fsSL https://raw.githubusercontent.com/prj786/ewe/main/get.sh) --yes"
            .to_string()
    };
    let script =
        format!("{inner}; s=$?; echo; read -n1 -s -p '— done (exit '$s') — press any key —'");
    for term in ["kitty", "foot", "alacritty", "xterm"] {
        if which(term) {
            Command::new(term)
                .args(["-e", "bash", "-lc", &script])
                .spawn()
                .map_err(estr)?;
            return Ok(());
        }
    }
    Err("no terminal emulator found (tried kitty, foot, alacritty, xterm)".into())
}
