//! Environment checks: fuse2 (needed to *launch* AppImages), pkexec, the
//! privileged helper, the build toolchain for AUR packages, and the GNOME
//! AppIndicator extension for the tray.

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Manager};

use crate::pacman;
use crate::util::which;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub fuse2: bool,
    pub pkexec: bool,
    pub pacman: bool,
    /// pacman-contrib — without it the Updates view has no safe way to ask
    /// "what is out of date" (see pacman::upgradable_inner)
    pub checkupdates: bool,
    /// base-devel + git: required before any AUR package can be built
    pub build_tools: bool,
    pub helper_installed: bool,
    pub desktop: String,
    pub gnome: bool,
    pub appindicator_ok: bool,
}

/// Arch keeps everything in /usr/lib — there is no Debian multiarch triplet
/// directory here, and the package is plain `fuse2`.
fn has_fuse2() -> bool {
    ["/usr/lib/libfuse.so.2", "/usr/lib64/libfuse.so.2"]
        .iter()
        .any(|p| Path::new(p).exists())
}

fn appindicator_extension_present(app: &AppHandle) -> bool {
    let mut dirs = vec![std::path::PathBuf::from(
        "/usr/share/gnome-shell/extensions",
    )];
    if let Ok(home) = app.path().home_dir() {
        dirs.push(home.join(".local/share/gnome-shell/extensions"));
    }
    dirs.iter().any(|dir| {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains("appindicator")
                })
            })
            .unwrap_or(false)
    })
}

#[tauri::command]
pub async fn system_check(app: AppHandle) -> Result<SystemInfo, String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let gnome = desktop.to_uppercase().contains("GNOME");
    Ok(SystemInfo {
        fuse2: has_fuse2(),
        pkexec: which("pkexec"),
        pacman: which("pacman"),
        checkupdates: which("checkupdates"),
        build_tools: which("makepkg") && which("git"),
        helper_installed: pacman::helper_installed(),
        gnome,
        appindicator_ok: !gnome || appindicator_extension_present(&app),
        desktop,
    })
}

/// One package name on Arch, no t64 variant to fall back to.
#[tauri::command]
pub async fn install_fuse2() -> Result<String, String> {
    pacman::install_package_named("fuse2").await
}

/// pacman-contrib provides `checkupdates` — without it the Updates view has no
/// safe way to list repo updates (see pacman::upgradable_inner), so the system
/// section sits silently empty. Offered as a one-click fix from that view.
#[tauri::command]
pub async fn install_pacman_contrib() -> Result<String, String> {
    pacman::install_package_named("pacman-contrib").await
}
