//! AppImage lifecycle: download → chmod +x → integrate (extract .desktop +
//! icon, no FUSE needed) → registry. All per-user, no root.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::catalog;
use crate::registry;
use crate::util::{client, estr, now_secs, slug};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub dir: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateItem {
    pub id: String,
    pub name: String,
    pub current: String,
    pub latest: String,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesResult {
    pub updates: Vec<UpdateItem>,
    pub errors: Vec<String>,
}

// ---------- paths ----------

fn home(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().home_dir().map_err(estr)
}

fn install_dir(app: &AppHandle, over: Option<String>) -> Result<PathBuf, String> {
    match over.filter(|s| !s.trim().is_empty()) {
        Some(d) => Ok(PathBuf::from(d)),
        None => Ok(home(app)?.join(".local/share/appimages")),
    }
}

fn applications_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home(app)?.join(".local/share/applications"))
}

fn icons_dir(app: &AppHandle, svg: bool) -> Result<PathBuf, String> {
    Ok(home(app)?.join(if svg {
        ".local/share/icons/hicolor/scalable/apps"
    } else {
        ".local/share/icons/hicolor/256x256/apps"
    }))
}

// ---------- progress ----------

fn emit_progress(app: &AppHandle, id: &str, phase: &str, downloaded: u64, total: u64) {
    let _ = app.emit(
        "install-progress",
        json!({ "id": id, "phase": phase, "downloaded": downloaded, "total": total }),
    );
}

pub async fn download_with_progress(
    app: &AppHandle,
    id: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let mut resp = client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(estr)?;
    }
    let mut file = tokio::fs::File::create(dest).await.map_err(estr)?;
    let mut downloaded: u64 = 0;
    let mut last = std::time::Instant::now();
    while let Some(chunk) = resp.chunk().await.map_err(estr)? {
        file.write_all(&chunk).await.map_err(estr)?;
        downloaded += chunk.len() as u64;
        if last.elapsed().as_millis() > 150 {
            last = std::time::Instant::now();
            emit_progress(app, id, "downloading", downloaded, total);
        }
    }
    file.flush().await.map_err(estr)?;
    emit_progress(app, id, "downloading", downloaded, total.max(downloaded));
    Ok(())
}

// ---------- desktop integration ----------

fn extract_field(content: &str, key: &str) -> Option<String> {
    let mut in_entry = false;
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(rest) = l.strip_prefix(key) {
                if let Some(v) = rest.strip_prefix('=') {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

fn find_squash_icon(squash: &Path, icon_name: Option<&str>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![squash.join(".DirIcon")];
    if let Some(n) = icon_name {
        candidates.push(squash.join(format!("{n}.png")));
        candidates.push(squash.join(format!("{n}.svg")));
    }
    if let Ok(entries) = fs::read_dir(squash) {
        for e in entries.flatten() {
            let p = e.path();
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("svg") {
                    candidates.push(p);
                }
            }
        }
    }
    candidates
        .into_iter()
        .find_map(|c| fs::canonicalize(&c).ok().filter(|p| p.is_file()))
}

async fn copy_remote_icon(
    app: &AppHandle,
    id: &str,
    icon_url: &str,
) -> Result<Option<PathBuf>, String> {
    let svg = icon_url.to_lowercase().ends_with(".svg");
    let dir = icons_dir(app, svg)?;
    fs::create_dir_all(&dir).map_err(estr)?;
    let dest = dir.join(format!("komble-{id}.{}", if svg { "svg" } else { "png" }));
    let resp = client().get(icon_url).send().await.map_err(estr)?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let bytes = resp.bytes().await.map_err(estr)?;
    fs::write(&dest, &bytes).map_err(estr)?;
    Ok(Some(dest))
}

/// Extract the AppImage (works without FUSE) and produce a desktop entry +
/// icon under ~/.local/share. Falls back to a synthesized entry.
async fn integrate(
    app: &AppHandle,
    id: &str,
    name: &str,
    version: &str,
    appimage_path: &Path,
    icon_url: Option<&str>,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let tmp = std::env::temp_dir().join(format!("komble-integrate-{id}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(estr)?;

    let extracted = tokio::process::Command::new(appimage_path)
        .arg("--appimage-extract")
        .current_dir(&tmp)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    let squash = tmp.join("squashfs-root");

    let mut orig_desktop = None;
    if extracted && squash.is_dir() {
        if let Ok(entries) = fs::read_dir(&squash) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("desktop") {
                    orig_desktop = fs::read_to_string(&p).ok();
                    break;
                }
            }
        }
    }

    // Icon: prefer one embedded in the AppImage, else the catalog icon.
    let orig = orig_desktop.as_deref().unwrap_or("");
    let orig_icon_name = extract_field(orig, "Icon");
    let mut icon_file: Option<PathBuf> = None;
    if extracted && squash.is_dir() {
        if let Some(src) = find_squash_icon(&squash, orig_icon_name.as_deref()) {
            let svg = src.extension().and_then(|e| e.to_str()) == Some("svg");
            let dir = icons_dir(app, svg)?;
            if fs::create_dir_all(&dir).is_ok() {
                let dest = dir.join(format!("komble-{id}.{}", if svg { "svg" } else { "png" }));
                if fs::copy(&src, &dest).is_ok() {
                    icon_file = Some(dest);
                }
            }
        }
    }
    if icon_file.is_none() {
        if let Some(url) = icon_url {
            icon_file = copy_remote_icon(app, id, url).await.unwrap_or(None);
        }
    }

    let icon_value = if icon_file.is_some() {
        format!("komble-{id}")
    } else {
        orig_icon_name.unwrap_or_else(|| "application-x-executable".into())
    };
    let display_name = extract_field(orig, "Name").unwrap_or_else(|| name.to_string());
    let comment = extract_field(orig, "Comment").unwrap_or_else(|| "Installed by Komble".into());
    let categories = extract_field(orig, "Categories").unwrap_or_else(|| "Utility;".into());
    let mime = extract_field(orig, "MimeType")
        .map(|m| format!("MimeType={m}\n"))
        .unwrap_or_default();

    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={display_name}\nComment={comment}\nExec=\"{exec}\" %U\nIcon={icon_value}\nTerminal=false\nCategories={categories}\n{mime}X-Komble-Id={id}\nX-AppImage-Version={version}\n",
        exec = appimage_path.to_string_lossy(),
    );

    let apps_dir = applications_dir(app)?;
    fs::create_dir_all(&apps_dir).map_err(estr)?;
    let desktop_path = apps_dir.join(format!("komble-{id}.desktop"));
    fs::write(&desktop_path, desktop).map_err(estr)?;

    let _ = tokio::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output()
        .await;
    let _ = fs::remove_dir_all(&tmp);
    Ok((desktop_path, icon_file))
}

// ---------- install / remove / list ----------

pub async fn install_inner(app: &AppHandle, req: InstallRequest) -> Result<Value, String> {
    let id = slug(&req.id);
    let version = req.version.clone().unwrap_or_else(|| "latest".into());
    let dir = install_dir(app, req.dir.clone())?;
    fs::create_dir_all(&dir).map_err(estr)?;

    let file_name = format!("{}-{}.AppImage", slug(&req.name), slug(&version));
    let tmp = dir.join(format!(".{file_name}.partial"));
    let dest = dir.join(&file_name);

    download_with_progress(app, &id, &req.url, &tmp).await?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(estr)?;
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    fs::rename(&tmp, &dest).map_err(estr)?;

    emit_progress(app, &id, "integrating", 0, 0);
    let (desktop_path, icon_file) = integrate(
        app,
        &id,
        &req.name,
        &version,
        &dest,
        req.icon_url.as_deref(),
    )
    .await?;

    let entry = json!({
        "id": id,
        "name": req.name,
        "version": version,
        "path": dest.to_string_lossy(),
        "repo": req.repo,
        "iconUrl": req.icon_url,
        "iconFile": icon_file.map(|p| p.to_string_lossy().into_owned()),
        "desktopFile": desktop_path.to_string_lossy(),
        "installedAt": now_secs(),
    });
    registry::upsert_appimage(app, &id, entry.clone())?;
    emit_progress(app, &id, "done", 0, 0);
    Ok(entry)
}

#[tauri::command]
pub async fn install_appimage(app: AppHandle, req: InstallRequest) -> Result<Value, String> {
    install_inner(&app, req).await
}

#[tauri::command]
pub async fn remove_appimage(app: AppHandle, id: String) -> Result<(), String> {
    let entry = registry::get_appimage(&app, &id)?
        .ok_or_else(|| "Not found in the Komble registry.".to_string())?;
    for key in ["path", "desktopFile", "iconFile"] {
        if let Some(p) = entry.get(key).and_then(|v| v.as_str()) {
            let _ = fs::remove_file(p);
        }
    }
    registry::remove_appimage(&app, &id)
}

#[tauri::command]
pub async fn list_appimages(app: AppHandle) -> Result<Vec<Value>, String> {
    registry::appimages(&app)
}

// ---------- local file install (drag & drop wizard) ----------

fn validate_appimage_path(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err("Path must be absolute.".into());
    }
    let p = p
        .canonicalize()
        .map_err(|_| "File not found.".to_string())?;
    let ok = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("appimage"))
        .unwrap_or(false);
    if !ok {
        return Err("Not an .AppImage file.".into());
    }
    if !p.is_file() {
        return Err("Not a regular file.".into());
    }
    Ok(p)
}

/// Guess "Name" and "1.2.3" from stems like `MyApp-1.2.3-x86_64`.
fn parse_appimage_name(stem: &str) -> (String, String) {
    const NOISE: &[&str] = &[
        "x86", "64", "x86_64", "x64", "amd64", "aarch64", "arm64", "armhf", "i386", "i686",
        "linux", "appimage", "portable", "release", "stable",
    ];
    let mut name_parts: Vec<&str> = Vec::new();
    let mut version = String::new();
    for t in stem.split(['-', '_']).filter(|t| !t.is_empty()) {
        let tl = t.to_lowercase();
        let bare = t.trim_start_matches(['v', 'V']);
        let is_ver = !bare.is_empty()
            && bare
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            && bare.chars().all(|c| c.is_ascii_digit() || c == '.');
        if version.is_empty() && is_ver && !name_parts.is_empty() {
            version = bare.to_string();
            continue;
        }
        if NOISE.contains(&tl.as_str()) {
            continue;
        }
        if version.is_empty() {
            name_parts.push(t);
        }
    }
    let name = if name_parts.is_empty() {
        stem.to_string()
    } else {
        name_parts.join(" ")
    };
    (
        name,
        if version.is_empty() {
            "local".into()
        } else {
            version
        },
    )
}

#[tauri::command]
pub async fn appimage_file_info(path: String) -> Result<Value, String> {
    let p = validate_appimage_path(&path)?;
    let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("App");
    let (name, version) = parse_appimage_name(stem);
    Ok(json!({
        "path": p.to_string_lossy(),
        "suggestedName": name,
        "suggestedVersion": version,
        "size": size,
    }))
}

#[tauri::command]
pub async fn install_local_appimage(
    app: AppHandle,
    path: String,
    name: String,
    version: Option<String>,
    dir: Option<String>,
) -> Result<Value, String> {
    let src = validate_appimage_path(&path)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("A name is required.".into());
    }
    let id = slug(&name);
    let version = version
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "local".into());
    let dirp = install_dir(&app, dir)?;
    fs::create_dir_all(&dirp).map_err(estr)?;
    let dest = dirp.join(format!("{}-{}.AppImage", slug(&name), slug(&version)));

    emit_progress(&app, &id, "copying", 0, 0);
    if src != dest {
        fs::copy(&src, &dest).map_err(estr)?;
    }
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o755)).map_err(estr)?;

    emit_progress(&app, &id, "integrating", 0, 0);
    let (desktop_path, icon_file) = integrate(&app, &id, &name, &version, &dest, None).await?;

    let entry = json!({
        "id": id,
        "name": name,
        "version": version,
        "path": dest.to_string_lossy(),
        "repo": null,
        "source": "local",
        "iconUrl": null,
        "iconFile": icon_file.map(|p| p.to_string_lossy().into_owned()),
        "desktopFile": desktop_path.to_string_lossy(),
        "installedAt": now_secs(),
    });
    registry::upsert_appimage(&app, &id, entry.clone())?;
    emit_progress(&app, &id, "done", 0, 0);
    Ok(entry)
}

// ---------- updates ----------

pub async fn check_updates_inner(
    app: &AppHandle,
    token: Option<String>,
) -> Result<UpdatesResult, String> {
    let mut updates = Vec::new();
    let mut errors = Vec::new();
    for entry in registry::appimages(app)? {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let current = entry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let Some(repo) = entry.get("repo").and_then(|v| v.as_str()) else {
            continue;
        };
        match catalog::resolve_release(app.clone(), repo.to_string(), token.clone()).await {
            Ok(rel) => {
                if !rel.version.is_empty() && rel.version != current {
                    updates.push(UpdateItem {
                        id,
                        name,
                        current,
                        latest: rel.version,
                        url: rel.assets[0].url.clone(),
                    });
                }
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    Ok(UpdatesResult { updates, errors })
}

#[tauri::command]
pub async fn check_appimage_updates(
    app: AppHandle,
    token: Option<String>,
) -> Result<UpdatesResult, String> {
    check_updates_inner(&app, token).await
}

#[tauri::command]
pub async fn update_appimage(
    app: AppHandle,
    id: String,
    token: Option<String>,
) -> Result<Value, String> {
    let entry = registry::get_appimage(&app, &id)?
        .ok_or_else(|| "Not found in the Komble registry.".to_string())?;
    let repo = entry
        .get("repo")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "No source repository recorded for this app.".to_string())?
        .to_string();
    let old_path = entry.get("path").and_then(|v| v.as_str()).map(String::from);
    let rel = catalog::resolve_release(app.clone(), repo.clone(), token).await?;
    let req = InstallRequest {
        id: id.clone(),
        name: entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string(),
        url: rel.assets[0].url.clone(),
        version: Some(rel.version),
        repo: Some(repo),
        icon_url: entry
            .get("iconUrl")
            .and_then(|v| v.as_str())
            .map(String::from),
        dir: old_path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().into_owned()),
    };
    let new_entry = install_inner(&app, req).await?;
    if let (Some(old), Some(new)) = (old_path, new_entry.get("path").and_then(|v| v.as_str())) {
        if old != new {
            let _ = fs::remove_file(old);
        }
    }
    Ok(new_entry)
}
