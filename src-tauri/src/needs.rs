//! What an installed app still needs before it can do its main job, and
//! that Komble can fetch for it with one click.
//!
//! Stremio Enhanced is the first (owner, 2026-11-21: "now i cant even play a
//! video from stremio"). It installs fine as an AppImage or from the AUR, but
//! on Linux it only looks for Stremio's streaming server as a Flatpak
//! (`flatpak info com.stremio.Service` — ewe has no Flatpak) or as
//! `server.js` in `$XDG_CONFIG_HOME/stremio-enhanced/streamingserver/`, which
//! it then runs itself with the system ffmpeg/ffprobe (ewe ships both). With
//! neither, torrents never start and nothing is transcoded, so most videos
//! never play. The file is Stremio's own (`dl.strem.io/server/<v>/desktop/
//! server.js`); its version comes from the `version.txt` the app writes there,
//! else from `[package.metadata.server]` in stremio-service's Cargo.toml —
//! the same place the app itself checks for updates.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::util::{client, estr};

const STREMIO_SERVER_ID: &str = "stremio-server";
const SERVER_URL: &str = "https://dl.strem.io/server";
const SERVICE_CARGO_TOML: &str =
    "https://raw.githubusercontent.com/Stremio/stremio-service/refs/heads/master/Cargo.toml";
/// server.js v4.21.1 is 6.7 MB; anything far outside this is not it.
const SERVER_MIN_BYTES: usize = 1_000_000;
const SERVER_MAX_BYTES: usize = 64_000_000;

/// The ids Stremio Enhanced arrives under: the AppImage catalog's slug and
/// the AUR packages.
const STREMIO_ENHANCED: &[&str] = &[
    "stremio-enhanced",
    "stremio-enhanced-bin",
    "stremio-enhanced-git",
];

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

fn stremio_server_dir() -> PathBuf {
    config_home()
        .join("stremio-enhanced")
        .join("streamingserver")
}

/// "v4.21.1" and nothing else: it ends up in a URL path.
fn valid_version(v: &str) -> bool {
    let Some(rest) = v.strip_prefix('v') else {
        return false;
    };
    v.len() <= 20
        && !rest.is_empty()
        && rest.split('.').count() >= 2
        && rest
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// The version line out of stremio-service's Cargo.toml:
/// `[package.metadata.server]` … `version = "v4.21.1"`.
fn version_from_cargo_toml(text: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[package.metadata.server]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(value) = line
            .strip_prefix("version")
            .map(str::trim_start)
            .and_then(|r| r.strip_prefix('='))
        {
            let v = value.trim().trim_matches('"').to_string();
            return valid_version(&v).then_some(v);
        }
    }
    None
}

/// A download that is really server.js: the right size and JavaScript, not
/// an HTML error page.
fn looks_like_server_js(bytes: &[u8]) -> bool {
    if !(SERVER_MIN_BYTES..=SERVER_MAX_BYTES).contains(&bytes.len()) {
        return false;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    !head.trim_start().starts_with('<')
}

fn stremio_server_ok(dir: &Path) -> bool {
    std::fs::metadata(dir.join("server.js"))
        .map(|m| m.is_file() && m.len() as usize >= SERVER_MIN_BYTES)
        .unwrap_or(false)
}

fn is_stremio_enhanced(id: &str) -> bool {
    STREMIO_ENHANCED.contains(&id)
}

/// What `id` (an AppImage id or a package name) still needs.
/// `{"needs": [{id, title, detail, action, ok}]}` — empty for most apps.
#[tauri::command]
pub fn app_needs(id: String) -> Value {
    let mut needs = Vec::new();
    if is_stremio_enhanced(id.trim()) {
        needs.push(json!({
            "id": STREMIO_SERVER_ID,
            "title": "Needs Stremio’s streaming server to play",
            "detail": "Stremio Enhanced looks for it as a Flatpak, which ewe doesn’t use. \
                       Komble can put Stremio’s own server (about 7 MB from strem.io) where \
                       the app finds it. Restart Stremio Enhanced afterwards.",
            "action": "Get the streaming server",
            "ok": stremio_server_ok(&stremio_server_dir()),
        }));
    }
    json!({ "needs": needs })
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let resp = client().get(url).send().await.map_err(estr)?;
    if !resp.status().is_success() {
        return Err(format!("{url} answered {}", resp.status()));
    }
    resp.text().await.map_err(estr)
}

async fn fix_stremio_server() -> Result<Value, String> {
    fix_stremio_server_in(&stremio_server_dir()).await
}

async fn fix_stremio_server_in(dir: &Path) -> Result<Value, String> {
    std::fs::create_dir_all(dir).map_err(estr)?;

    let pinned = std::fs::read_to_string(dir.join("version.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|v| valid_version(v));
    let version = match pinned {
        Some(v) => v,
        None => version_from_cargo_toml(&fetch_text(SERVICE_CARGO_TOML).await?)
            .ok_or("Couldn’t find which streaming server version Stremio uses")?,
    };

    let url = format!("{SERVER_URL}/{version}/desktop/server.js");
    let resp = client().get(&url).send().await.map_err(estr)?;
    if !resp.status().is_success() {
        return Err(format!("strem.io answered {} for {version}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(estr)?;
    if !looks_like_server_js(&bytes) {
        return Err("The download wasn’t Stremio’s streaming server".into());
    }

    // write beside, then rename: the app never sees half a file
    let tmp = dir.join(".server.js.komble");
    std::fs::write(&tmp, &bytes).map_err(estr)?;
    std::fs::rename(&tmp, dir.join("server.js")).map_err(estr)?;
    std::fs::write(dir.join("version.txt"), &version).map_err(estr)?;
    Ok(json!({ "ok": true, "need": STREMIO_SERVER_ID, "version": version, "bytes": bytes.len() }))
}

/// Fetch what `app_needs` said is missing. `{"ok":true, …}` or an error string.
#[tauri::command]
pub async fn fix_app_need(need: String) -> Result<Value, String> {
    match need.as_str() {
        STREMIO_SERVER_ID => fix_stremio_server().await,
        other => Err(format!("unknown need: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(valid_version("v4.21.1"));
        assert!(valid_version("v4.20"));
        assert!(!valid_version("4.21.1"));
        assert!(!valid_version("v4..1"));
        assert!(!valid_version("v4.21.1/../../x"));
        assert!(!valid_version("v"));
        assert!(!valid_version("vlatest"));
    }

    #[test]
    fn cargo_toml_version() {
        let toml = "[package]\nversion = \"0.1.22\"\n\n[package.metadata.server]\nversion = \"v4.21.1\"\n\n[features]\n";
        assert_eq!(version_from_cargo_toml(toml).as_deref(), Some("v4.21.1"));
        // the package's own version is not the server's
        assert_eq!(
            version_from_cargo_toml("[package]\nversion = \"0.1.22\"\n"),
            None
        );
        assert_eq!(
            version_from_cargo_toml("[package.metadata.server]\nversion = \"latest\"\n"),
            None
        );
    }

    #[test]
    fn server_js_shape() {
        let js: Vec<u8> = b"(function(){"
            .iter()
            .copied()
            .cycle()
            .take(2_000_000)
            .collect();
        assert!(looks_like_server_js(&js));
        let html: Vec<u8> = b"<!DOCTYPE html>"
            .iter()
            .copied()
            .cycle()
            .take(2_000_000)
            .collect();
        assert!(!looks_like_server_js(&html));
        assert!(!looks_like_server_js(b"tiny"));
    }

    #[test]
    fn only_stremio_enhanced_has_needs() {
        assert_eq!(
            app_needs("firefox".into())["needs"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let n = app_needs("stremio-enhanced".into());
        assert_eq!(n["needs"][0]["id"], STREMIO_SERVER_ID);
        let n = app_needs("stremio-enhanced-bin".into());
        assert_eq!(n["needs"].as_array().unwrap().len(), 1);
    }

    /// Real download from strem.io into a temp folder (network):
    /// `cargo test -- --ignored stremio_download`.
    #[tokio::test]
    #[ignore]
    async fn stremio_download() {
        let tmp = std::env::temp_dir().join(format!("komble-needs-dl-{}", std::process::id()));
        let r = fix_stremio_server_in(&tmp).await.unwrap();
        assert_eq!(r["ok"], true);
        assert!(stremio_server_ok(&tmp));
        let v = std::fs::read_to_string(tmp.join("version.txt")).unwrap();
        assert!(valid_version(&v), "{v}");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn server_present_is_ok() {
        let tmp = std::env::temp_dir().join(format!("komble-needs-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(!stremio_server_ok(&tmp));
        std::fs::write(tmp.join("server.js"), vec![b'x'; SERVER_MIN_BYTES]).unwrap();
        assert!(stremio_server_ok(&tmp));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
