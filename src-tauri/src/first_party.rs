//! First-party ewe apps (Komble itself, ewe-settings, ewe-sync, ewe-cast)
//! and the desktop update.
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
    (
        "ewe-sync",
        "prj786/ewe-sync",
        "Your ewe account — the one file, your machines, your folders",
    ),
    // Headless on purpose (RFC-004): the shell's Cast card is the UI, this is
    // the daemon behind it. Listed anyway — it is a first-party package a user
    // can install, update and remove like any other.
    (
        "ewe-cast",
        "prj786/ewe-cast",
        "Cast the ewe desktop to a TV — Miracast and Chromecast (daemon)",
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

/// tag + the machine-matching .pkg.tar.zst asset URL from a repo's NEWEST
/// GitHub release (same selection rule as the installer: skip -debug-, take
/// this arch or -any).
///
/// Newest, not GitHub's "Latest": /releases/latest skips prereleases, so for
/// as long as every release carries -beta it answered with the last stable
/// tag (v0.11.2) — nothing was ever newer than the installed 0.12.x beta and
/// every row sat on a green tick while the [ewe] repo, which tracks the most
/// recent release like ewe-repo's publish does, listed the same package as
/// a pending system update. The release list is date-ordered; drafts are
/// skipped.
async fn latest_release(
    repo: &str,
    token: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=10");
    let mut req = crate::util::client().get(&url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let res = req.send().await.map_err(estr)?;
    if !res.status().is_success() {
        return Err(format!("{repo}: GitHub API HTTP {}", res.status()));
    }
    let j: Value = res.json().await.map_err(estr)?;
    let rel = j
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["draft"] != true && !r["tag_name"].as_str().unwrap_or("").is_empty())
        .ok_or_else(|| format!("{repo}: no release found"))?;
    let tag = rel["tag_name"].as_str().unwrap_or_default();
    let version = tag.trim_start_matches('v').to_string();
    let arch = std::env::consts::ARCH; // x86_64 / aarch64 — matches uname -m
    let asset = rel["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["browser_download_url"].as_str())
        .filter(|u| u.ends_with(".pkg.tar.zst") && !u.contains("-debug-"))
        .find(|u| u.contains(arch) || u.contains("-any.pkg"))
        .map(String::from);
    Ok((version, asset))
}

/// A pending repo update as the frontend already learned it from
/// checkupdates (`list_upgradable`) — passed in rather than re-run here, so
/// the two views can never disagree and no second checkupdates races the
/// first on the private sync DB.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoUpdate {
    pub name: String,
    #[serde(default)]
    pub latest: String,
}

/// "0.12.1beta-1" → "0.12.1beta": pacman's pkgrel is noise next to a release
/// tag, and the desktop's own row reads without it.
fn strip_pkgrel(v: &str) -> String {
    match v.rsplit_once('-') {
        Some((base, rel))
            if !rel.is_empty() && rel.chars().all(|c| c.is_ascii_digit() || c == '.') =>
        {
            base.to_string()
        }
        _ => v.to_string(),
    }
}

/// The version a sync repo ([ewe]) offers for this package, None when no
/// enabled repo carries it. When it does, pacman owns the package: `pacman
/// -S` installs it, `-Syu` updates it, and a GitHub asset dropped on top
/// with `pacman -U` would be a partial upgrade — so the release is never
/// consulted. Reads the local sync DB only (no network, no -Sy).
async fn sync_version(pkg: &str) -> Option<String> {
    let out = Command::new("pacman")
        .args(["-Si", pkg])
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("Version"))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// One row per first-party app: installed version (None = not installed),
/// what is available, and whether an update is pending.
///
/// `managed` says who delivers it: "repo" — a sync repo carries the package
/// and the pending version comes from the checkupdates list the frontend
/// passes in (the update IS the system upgrade); "release" — no repo knows
/// it (a get.sh / developer install), so the newest GitHub release is the
/// reference and `pacman -U` of its asset is the update.
#[tauri::command]
pub async fn first_party_status(
    token: Option<String>,
    repo_updates: Option<Vec<RepoUpdate>>,
) -> Result<Vec<Value>, String> {
    let updates = repo_updates.unwrap_or_default();
    let mut rows = Vec::new();
    for (pkg, repo, summary) in DISCOVER {
        // display without the pacman pkgrel: the row reads "0.9.2" like the
        // desktop's own row, not "0.9.2-1" (comparisons still use vercmp on
        // full strings elsewhere)
        let installed = crate::pacman::installed_version(pkg)
            .await
            .map(|v| strip_pkgrel(&v));

        if let Some(sync) = sync_version(pkg).await {
            let pending = updates.iter().find(|u| u.name == *pkg);
            let latest = pending
                .map(|u| strip_pkgrel(&u.latest))
                .or_else(|| installed.clone())
                .unwrap_or_else(|| strip_pkgrel(&sync));
            rows.push(json!({
                "pkg": pkg, "repo": repo, "summary": summary,
                "installed": installed,
                "latest": latest,
                "hasAsset": true,
                "managed": "repo",
                "updateAvailable": installed.is_some() && pending.is_some(),
            }));
            continue;
        }

        let (latest, asset) = match latest_release(repo, token.as_deref()).await {
            Ok(x) => x,
            Err(e) => {
                rows.push(json!({
                    "pkg": pkg, "repo": repo, "summary": summary,
                    "installed": installed, "latest": Value::Null,
                    "managed": "release",
                    "updateAvailable": false, "error": e,
                }));
                continue;
            }
        };
        let cur_base = installed.clone().unwrap_or_default();
        rows.push(json!({
            "pkg": pkg, "repo": repo, "summary": summary,
            "installed": installed,
            "latest": latest,
            "hasAsset": asset.is_some(),
            "managed": "release",
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

    // a sync repo carries it: install it like any other repo package, never
    // a release asset on top of what pacman already tracks
    if sync_version(&pkg).await.is_some() {
        let _ = app.emit("install-progress", json!({ "id": pkg, "stage": "install" }));
        return crate::pacman::install_package(app.clone(), pkg.clone()).await;
    }

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
    crate::pacman::install_package_file_as(
        app.clone(),
        dest.to_string_lossy().to_string(),
        "first-party",
    )
    .await
}

/// The desktop itself: where it lives, what version it is, and whether the
/// repo's update.sh says there is anything to pull. `git: false` means a
/// tarball (get.sh) install — update then means "re-run get.sh".
///
/// `packaged: true` means the `ewe` pacman package (the [ewe] repo, what the
/// ISO installs): the version is the payload's, the pending one comes from
/// the checkupdates list the frontend passes in, and the update is the
/// system upgrade — there is nothing to pull and nothing to re-run.
#[tauri::command]
pub async fn ewe_status(
    token: Option<String>,
    repo_updates: Option<Vec<RepoUpdate>>,
) -> Result<Value, String> {
    let dir = ewe_dir();
    let is_git = dir.join(".git").is_dir() && dir.join("update.sh").is_file();

    if !is_git {
        if let Some(pkgver) = crate::pacman::installed_version("ewe").await {
            let payload = PathBuf::from("/usr/share/ewe");
            let version = std::fs::read_to_string(payload.join("VERSION"))
                .map(|s| s.trim().to_string())
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| strip_pkgrel(&pkgver));
            let pending = repo_updates
                .unwrap_or_default()
                .into_iter()
                .find(|u| u.name == "ewe");
            let latest = pending
                .as_ref()
                .map(|u| strip_pkgrel(&u.latest))
                .unwrap_or_else(|| version.clone());
            return Ok(json!({
                "installed": true, "packaged": true, "git": false,
                "dir": payload.to_string_lossy(),
                "version": version, "latest": latest,
                "updateAvailable": pending.is_some(),
                "dirty": false,
            }));
        }
    }

    let version = std::fs::read_to_string(dir.join("VERSION"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if version.is_empty() {
        return Ok(json!({ "installed": false }));
    }

    if is_git {
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
    if !dir.join(".git").is_dir() && crate::pacman::installed_version("ewe").await.is_some() {
        return Err(
            "The desktop is a pacman package here — it updates with the system upgrade.".into(),
        );
    }
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
    } else if crate::pacman::installed_version("ewe").await.is_some() {
        // the [ewe] repo delivers the desktop: a system upgrade is the update
        "sudo pacman -Syu".to_string()
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
