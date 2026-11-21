//! "Apps you picked while installing ewe" (ewe-os 0.17).
//!
//! The installer's Apps screen offers this app's curated list. Repo apps
//! install with the system; the installer never builds from the AUR or
//! fetches an AppImage, so those picks — and any repo app that could not be
//! installed then — are left for Komble in the new user's
//! `$XDG_STATE_HOME/ewe/install-picks.json` (default `~/.local/state/ewe/`):
//!
//! ```json
//! {"version": 1, "at": "2026-11-21T12:00:00Z",
//!  "picked": [{"pkg": "spotify", "source": "aur"},
//!             {"pkg": "lmstudio", "source": "appimage"}]}
//! ```
//!
//! `source` is `aur`, `appimage` (an AM catalog slug) or `repo`. For you shows
//! them with an Install button (repo, AppImage) or a Review one (AUR — the
//! PKGBUILD review gate stays), until each is installed (dropped from the file
//! on the next read) or dismissed (dropped at once). The last one gone takes
//! the file with it. Komble only ever removes entries; the installer writes
//! the file (ewe-os `installer/helper/ewe-install-helper`, verb `apps`).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const FILE: &str = "ewe/install-picks.json";

fn picks_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
            PathBuf::from(home).join(".local/state")
        });
    state.join(FILE)
}

/// The package names Arch allows (and the AM slugs the curated list uses).
pub(crate) fn pkg_ok(p: &str) -> bool {
    let mut chars = p.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() || "@_+".contains(c))
        && p.len() <= 100
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "@._+-".contains(c))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pick {
    pub pkg: String,
    pub source: String,
}

/// The file's picks, each valid one once; anything else in it is ignored.
pub(crate) fn parse(text: &str) -> (String, Vec<Pick>) {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let at = v["at"].as_str().unwrap_or("").to_string();
    let mut out: Vec<Pick> = Vec::new();
    for p in v["picked"].as_array().cloned().unwrap_or_default() {
        let pkg = p["pkg"].as_str().unwrap_or("");
        let source = p["source"].as_str().unwrap_or("");
        if pkg_ok(pkg)
            && ["aur", "appimage", "repo"].contains(&source)
            && !out.iter().any(|x| x.pkg == pkg)
        {
            out.push(Pick {
                pkg: pkg.into(),
                source: source.into(),
            });
        }
    }
    (at, out)
}

/// Write what is left, or remove the file when nothing is (atomic).
pub(crate) fn store(path: &Path, at: &str, picks: &[Pick]) -> Result<(), String> {
    if picks.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let body = json!({
        "version": 1,
        "at": at,
        "picked": picks.iter().map(|p| json!({"pkg": p.pkg, "source": p.source})).collect::<Vec<_>>(),
    });
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(&body).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// The picks still waiting: `installed(pick)` says which ones are here now;
/// those leave the file for good (so one removed later never comes back).
pub(crate) fn settle(
    path: &Path,
    installed: impl Fn(&Pick) -> bool,
) -> Option<(String, Vec<Pick>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (at, all) = parse(&text);
    let left: Vec<Pick> = all.iter().filter(|p| !installed(p)).cloned().collect();
    if left.len() != all.len() {
        let _ = store(path, &at, &left);
    }
    Some((at, left))
}

#[tauri::command]
pub async fn install_picks(app: tauri::AppHandle) -> Result<Value, String> {
    let path = picks_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(json!({"available": false}));
    };
    // what is here: pacman for packages, Komble's registry for AppImages
    let (_, all) = parse(&text);
    let mut have: Vec<String> = Vec::new();
    let appimages: Vec<String> = crate::registry::appimages(&app)
        .unwrap_or_default()
        .iter()
        .filter_map(|a| a["id"].as_str().map(String::from))
        .collect();
    for p in &all {
        let here = if p.source == "appimage" {
            appimages.contains(&p.pkg)
        } else {
            crate::pacman::installed_version(&p.pkg).await.is_some()
        };
        if here {
            have.push(p.pkg.clone());
        }
    }
    let Some((at, left)) = settle(&path, |p| have.contains(&p.pkg)) else {
        return Ok(json!({"available": false}));
    };
    Ok(json!({
        "available": !left.is_empty(),
        "at": at,
        "picked": left.iter().map(|p| json!({"pkg": p.pkg, "source": p.source})).collect::<Vec<_>>(),
    }))
}

/// "Not now" on one pick, or on all of them (`pkg` = null).
#[tauri::command]
pub async fn install_picks_dismiss(pkg: Option<String>) -> Result<Value, String> {
    let path = picks_path();
    if let Some(p) = pkg.as_deref() {
        if !pkg_ok(p) {
            return Err("not a package name".into());
        }
    }
    let Some((_, left)) = settle(&path, |x| pkg.as_deref().is_none_or(|p| p == x.pkg)) else {
        return Ok(json!({"ok": true, "left": 0}));
    };
    Ok(json!({"ok": true, "left": left.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("komble-picks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("install-picks.json")
    }

    #[test]
    fn the_file_format() {
        let (at, p) = parse(
            r#"{"version":1,"at":"2026-11-21T12:00:00Z","picked":[
                {"pkg":"spotify","source":"aur"},{"pkg":"lmstudio","source":"appimage"},
                {"pkg":"vlc","source":"repo"},{"pkg":"spotify","source":"aur"},
                {"pkg":"Bad Name","source":"aur"},{"pkg":"x","source":"flatpak"},{"source":"aur"}]}"#,
        );
        assert_eq!(at, "2026-11-21T12:00:00Z");
        let names: Vec<&str> = p.iter().map(|x| x.pkg.as_str()).collect();
        assert_eq!(names, ["spotify", "lmstudio", "vlc"]);
        assert_eq!(p[1].source, "appimage");
        assert!(parse("not json").1.is_empty());
        assert!(parse("{}").1.is_empty());
    }

    #[test]
    fn names() {
        for ok in [
            "spotify",
            "0ad",
            "stremio-linux-shell",
            "libreoffice-fresh",
            "gtk+3",
            "lib32-mesa",
        ] {
            assert!(pkg_ok(ok), "{ok}");
        }
        for bad in ["", "-rf", "a b", "Spotify", "../x", "a;b"] {
            assert!(!pkg_ok(bad), "{bad}");
        }
    }

    #[test]
    fn installed_picks_leave_and_the_last_takes_the_file() {
        let path = tmp("settle");
        std::fs::write(
            &path,
            r#"{"version":1,"at":"t","picked":[{"pkg":"spotify","source":"aur"},{"pkg":"vlc","source":"repo"}]}"#,
        )
        .unwrap();
        let (_, left) = settle(&path, |p| p.pkg == "vlc").unwrap();
        assert_eq!(
            left,
            vec![Pick {
                pkg: "spotify".into(),
                source: "aur".into()
            }]
        );
        // the file now holds only what is left, and keeps its time
        let (at, again) = parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!((at.as_str(), again.len()), ("t", 1));
        let (_, none) = settle(&path, |_| true).unwrap();
        assert!(none.is_empty());
        assert!(!path.exists());
        assert!(settle(&path, |_| false).is_none());
    }

    #[test]
    fn nothing_installed_leaves_the_file_alone() {
        let path = tmp("untouched");
        let body = r#"{"version":1,"picked":[{"pkg":"spotify","source":"aur"}],"extra":"kept"}"#;
        std::fs::write(&path, body).unwrap();
        let (_, left) = settle(&path, |_| false).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
    }
}
