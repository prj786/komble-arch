//! AppImage lifecycle: download → chmod +x → integrate (extract the tree once,
//! no FUSE needed) → registry. All per-user, no root.
//!
//! Launch speed: an AppImage FUSE-mounts its squashfs and decompresses on
//! every read, so Chrome took 1.0–1.2 s to its first window; the same binary
//! run from an extracted tree takes 0.4 s. Komble therefore keeps the tree
//! `--appimage-extract` produces, next to the file, and points the desktop
//! entry's Exec at its `AppRun`. The .AppImage itself stays (updates and
//! portability use it); the tree is a cache that is rebuilt whenever the file
//! changes (see `ensure_run_dirs`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::catalog;
use crate::registry;
use crate::util::{client, estr, now_secs, slug};

/// Hidden directory inside the install dir that holds one extracted tree per
/// app id: `<install dir>/.komble-run/<id>/AppRun`. Same filesystem as the
/// .AppImage, so the swap on update is a rename and never touches /tmp (a
/// tmpfs here — Chrome's tree is 441 MB).
pub(crate) const RUN_ROOT: &str = ".komble-run";

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

/// Where the desktop entry and icons go. A plain struct so the filesystem
/// part of integration can run (and be tested) without an AppHandle.
#[derive(Clone, Debug)]
pub(crate) struct Dirs {
    pub applications: PathBuf,
    pub icons_png: PathBuf,
    pub icons_svg: PathBuf,
}

impl Dirs {
    fn icons(&self, svg: bool) -> &Path {
        if svg {
            &self.icons_svg
        } else {
            &self.icons_png
        }
    }
}

fn dirs(app: &AppHandle) -> Result<Dirs, String> {
    let h = home(app)?;
    Ok(Dirs {
        applications: h.join(".local/share/applications"),
        icons_png: h.join(".local/share/icons/hicolor/256x256/apps"),
        icons_svg: h.join(".local/share/icons/hicolor/scalable/apps"),
    })
}

/// `<install dir>/.komble-run` for the AppImage at `appimage_path`.
pub(crate) fn run_root_for(appimage_path: &Path) -> PathBuf {
    appimage_path
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join(RUN_ROOT)
}

/// `<install dir>/.komble-run/<id>` — the tree the desktop entry executes.
pub(crate) fn run_dir_for(appimage_path: &Path, id: &str) -> PathBuf {
    run_root_for(appimage_path).join(id)
}

/// A path we are willing to `remove_dir_all`: a direct child of a
/// `.komble-run` directory. Guards against a corrupted registry pointing the
/// uninstall at something that is not ours.
pub(crate) fn is_run_dir(p: &Path) -> bool {
    p.is_absolute()
        && p.file_name().is_some()
        && p.parent()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            == Some(RUN_ROOT)
}

/// Identity of the .AppImage file the run dir was extracted from. The tree is
/// stale when this changes (a new version, or the app replaced its own file
/// through $APPIMAGE as Electron's updater does).
pub(crate) fn file_stamp(appimage_path: &Path) -> Option<String> {
    let m = fs::metadata(appimage_path).ok()?;
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(format!("{}-{}", m.len(), mtime))
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

// ---------- desktop entry text ----------

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

/// One Exec argument, quoted per the Desktop Entry spec: double quotes around
/// the whole argument; `"`, `` ` ``, `$` and `\` escaped for the Exec parser,
/// then the backslash doubled again for the string-value level (the spec's
/// "four backslashes for one" rule); `%` doubled so it is not a field code.
pub(crate) fn exec_arg(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '`' | '$' => {
                out.push_str("\\\\");
                out.push(c);
            }
            '\\' => out.push_str("\\\\\\\\"),
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What the desktop entry launches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExecTarget {
    /// The .AppImage itself (FUSE mount on every launch) — the fallback for
    /// AppImages that refuse to extract or ship no executable AppRun.
    AppImage(PathBuf),
    /// The extracted tree's AppRun, with $APPIMAGE/$APPDIR set as the AppImage
    /// runtime would have set them.
    RunDir { appimage: PathBuf, run_dir: PathBuf },
}

impl ExecTarget {
    /// The `Exec=` value (without the key).
    pub(crate) fn exec_line(&self) -> String {
        match self {
            ExecTarget::AppImage(p) => format!("{} %U", exec_arg(&p.to_string_lossy())),
            ExecTarget::RunDir { appimage, run_dir } => format!(
                "env {} {} {} %U",
                exec_arg(&format!("APPIMAGE={}", appimage.to_string_lossy())),
                exec_arg(&format!("APPDIR={}", run_dir.to_string_lossy())),
                exec_arg(&run_dir.join("AppRun").to_string_lossy()),
            ),
        }
    }

    /// The executable `TryExec=` should probe, when its path is representable
    /// as a plain string value (no backslash or line break to escape).
    pub(crate) fn try_exec(&self) -> Option<String> {
        let p = match self {
            ExecTarget::AppImage(p) => p.clone(),
            ExecTarget::RunDir { run_dir, .. } => run_dir.join("AppRun"),
        };
        let s = p.to_string_lossy().into_owned();
        if s.contains(['\\', '\n', '\r']) {
            None
        } else {
            Some(s)
        }
    }
}

pub(crate) struct DesktopSpec<'a> {
    pub id: &'a str,
    pub version: &'a str,
    /// The .desktop shipped inside the AppImage, if any — Name, Comment,
    /// Categories, MimeType and Icon are taken from it.
    pub original: &'a str,
    pub fallback_name: &'a str,
    /// Whether Komble installed an icon as `komble-<id>`.
    pub has_icon_file: bool,
    pub target: &'a ExecTarget,
}

pub(crate) fn build_desktop_entry(spec: &DesktopSpec) -> String {
    let orig = spec.original;
    let id = spec.id;
    let icon_value = if spec.has_icon_file {
        format!("komble-{id}")
    } else {
        extract_field(orig, "Icon").unwrap_or_else(|| "application-x-executable".into())
    };
    let display_name =
        extract_field(orig, "Name").unwrap_or_else(|| spec.fallback_name.to_string());
    let comment = extract_field(orig, "Comment").unwrap_or_else(|| "Installed by Komble".into());
    let categories = extract_field(orig, "Categories").unwrap_or_else(|| "Utility;".into());
    let mime = extract_field(orig, "MimeType")
        .map(|m| format!("MimeType={m}\n"))
        .unwrap_or_default();
    let try_exec = spec
        .target
        .try_exec()
        .map(|t| format!("TryExec={t}\n"))
        .unwrap_or_default();
    format!(
        "[Desktop Entry]\nType=Application\nName={display_name}\nComment={comment}\n{try_exec}Exec={exec}\nIcon={icon_value}\nTerminal=false\nCategories={categories}\n{mime}X-Komble-Id={id}\nX-AppImage-Version={version}\n",
        exec = spec.target.exec_line(),
        version = spec.version,
    )
}

// ---------- the extracted tree ----------

fn apprun_is_executable(tree: &Path) -> bool {
    fs::metadata(tree.join("AppRun"))
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Remove work dirs and swapped-out trees a crashed earlier run left for `id`.
fn sweep_leftovers(run_root: &Path, id: &str) {
    let Ok(entries) = fs::read_dir(run_root) else {
        return;
    };
    let work = format!(".extract-{id}-");
    let old = format!(".old-{id}-");
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(n) = name.to_str() else { continue };
        if n.starts_with(&work) || n.starts_with(&old) {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

/// Run `<appimage> --appimage-extract` inside `<run root>/.extract-<id>-<pid>`
/// and swap the resulting `squashfs-root` into `<run root>/<id>` with renames
/// (same filesystem, so atomic — a launcher sees the old tree or the new one,
/// never a half-written one). Returns the run dir, or None when the AppImage
/// refuses to extract or ships no executable AppRun — nothing is left behind
/// in that case and the caller falls back to running the .AppImage.
pub(crate) fn extract_run_tree(appimage_path: &Path, run_root: &Path, id: &str) -> Option<PathBuf> {
    fs::create_dir_all(run_root).ok()?;
    sweep_leftovers(run_root, id);
    let nonce = std::process::id();
    let work = run_root.join(format!(".extract-{id}-{nonce}"));
    fs::create_dir_all(&work).ok()?;

    let extracted = std::process::Command::new(appimage_path)
        .arg("--appimage-extract")
        .current_dir(&work)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let squash = work.join("squashfs-root");
    if !extracted || !squash.is_dir() || !apprun_is_executable(&squash) {
        let _ = fs::remove_dir_all(&work);
        return None;
    }

    let final_dir = run_root.join(id);
    let old = run_root.join(format!(".old-{id}-{nonce}"));
    if final_dir.exists() && fs::rename(&final_dir, &old).is_err() {
        let _ = fs::remove_dir_all(&work);
        return None;
    }
    if fs::rename(&squash, &final_dir).is_err() {
        // put the previous tree back; the app keeps launching as before
        let _ = fs::rename(&old, &final_dir);
        let _ = fs::remove_dir_all(&work);
        return None;
    }
    let _ = fs::remove_dir_all(&old);
    let _ = fs::remove_dir_all(&work);
    Some(final_dir)
}

fn find_tree_desktop(tree: &Path) -> Option<String> {
    for e in fs::read_dir(tree).ok()?.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("desktop") {
            if let Ok(s) = fs::read_to_string(&p) {
                return Some(s);
            }
        }
    }
    None
}

fn find_tree_icon(tree: &Path, icon_name: Option<&str>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![tree.join(".DirIcon")];
    if let Some(n) = icon_name {
        candidates.push(tree.join(format!("{n}.png")));
        candidates.push(tree.join(format!("{n}.svg")));
    }
    if let Ok(entries) = fs::read_dir(tree) {
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

/// Result of the filesystem half of integration (no network).
pub(crate) struct Staged {
    /// `<install dir>/.komble-run/<id>` when the tree is usable.
    pub run_dir: Option<PathBuf>,
    /// The .desktop found inside the tree ("" when none).
    pub original_desktop: String,
    /// The icon copied out of the tree into the icon theme, if one was found.
    pub icon_file: Option<PathBuf>,
}

/// Extract the tree and lift the icon out of it. Blocking (Chrome's tree is
/// 441 MB and takes seconds) — callers run it off the async runtime.
pub(crate) fn stage(dirs: &Dirs, id: &str, appimage_path: &Path) -> Staged {
    let run_root = run_root_for(appimage_path);
    let run_dir = extract_run_tree(appimage_path, &run_root, id);
    let Some(tree) = run_dir.as_deref() else {
        return Staged {
            run_dir: None,
            original_desktop: String::new(),
            icon_file: None,
        };
    };
    let original_desktop = find_tree_desktop(tree).unwrap_or_default();
    let icon_name = extract_field(&original_desktop, "Icon");
    let mut icon_file = None;
    if let Some(src) = find_tree_icon(tree, icon_name.as_deref()) {
        let svg = src.extension().and_then(|e| e.to_str()) == Some("svg");
        let dir = dirs.icons(svg);
        if fs::create_dir_all(dir).is_ok() {
            let dest = dir.join(format!("komble-{id}.{}", if svg { "svg" } else { "png" }));
            if fs::copy(&src, &dest).is_ok() {
                icon_file = Some(dest);
            }
        }
    }
    Staged {
        run_dir,
        original_desktop,
        icon_file,
    }
}

/// Write `komble-<id>.desktop` for the staged tree (or the .AppImage when
/// there is none). Returns the desktop file's path.
pub(crate) fn write_desktop_entry(
    dirs: &Dirs,
    id: &str,
    name: &str,
    version: &str,
    appimage_path: &Path,
    staged: &Staged,
    has_icon_file: bool,
) -> Result<PathBuf, String> {
    let target = match &staged.run_dir {
        Some(run_dir) => ExecTarget::RunDir {
            appimage: appimage_path.to_path_buf(),
            run_dir: run_dir.clone(),
        },
        None => ExecTarget::AppImage(appimage_path.to_path_buf()),
    };
    let desktop = build_desktop_entry(&DesktopSpec {
        id,
        version,
        original: &staged.original_desktop,
        fallback_name: name,
        has_icon_file,
        target: &target,
    });
    fs::create_dir_all(&dirs.applications).map_err(estr)?;
    let desktop_path = dirs.applications.join(format!("komble-{id}.desktop"));
    fs::write(&desktop_path, desktop).map_err(estr)?;
    Ok(desktop_path)
}

async fn copy_remote_icon(
    dirs: &Dirs,
    id: &str,
    icon_url: &str,
) -> Result<Option<PathBuf>, String> {
    let svg = icon_url.to_lowercase().ends_with(".svg");
    let dir = dirs.icons(svg);
    fs::create_dir_all(dir).map_err(estr)?;
    let dest = dir.join(format!("komble-{id}.{}", if svg { "svg" } else { "png" }));
    let resp = client().get(icon_url).send().await.map_err(estr)?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let bytes = resp.bytes().await.map_err(estr)?;
    fs::write(&dest, &bytes).map_err(estr)?;
    Ok(Some(dest))
}

/// Everything integration produced, as the registry records it.
pub(crate) struct Integrated {
    pub desktop_path: PathBuf,
    pub icon_file: Option<PathBuf>,
    pub run_dir: Option<PathBuf>,
    pub run_stamp: Option<String>,
}

/// Extract the AppImage next to itself (works without FUSE), keep the tree as
/// the run target, and produce a desktop entry + icon under ~/.local/share.
/// Falls back to an entry that runs the .AppImage when the tree is unusable.
async fn integrate(
    app: &AppHandle,
    id: &str,
    name: &str,
    version: &str,
    appimage_path: &Path,
    icon_url: Option<&str>,
    keep_icon: Option<PathBuf>,
) -> Result<Integrated, String> {
    let d = dirs(app)?;
    // stamp first: a file replaced while we extract is caught on the next pass
    let run_stamp = file_stamp(appimage_path);
    let staged = {
        let d = d.clone();
        let id = id.to_string();
        let p = appimage_path.to_path_buf();
        tokio::task::spawn_blocking(move || stage(&d, &id, &p))
            .await
            .map_err(estr)?
    };

    // Icon: the one embedded in the AppImage, else the one we already had
    // (re-integration), else the catalog's.
    let mut icon_file = staged.icon_file.clone();
    if icon_file.is_none() {
        icon_file = keep_icon.filter(|p| p.is_file());
    }
    if icon_file.is_none() {
        if let Some(url) = icon_url {
            icon_file = copy_remote_icon(&d, id, url).await.unwrap_or(None);
        }
    }

    let desktop_path = write_desktop_entry(
        &d,
        id,
        name,
        version,
        appimage_path,
        &staged,
        icon_file.is_some(),
    )?;
    let _ = tokio::process::Command::new("update-desktop-database")
        .arg(&d.applications)
        .output()
        .await;
    Ok(Integrated {
        desktop_path,
        icon_file,
        run_dir: staged.run_dir,
        run_stamp,
    })
}

/// Where an install came from, for the registry entry.
struct Origin<'a> {
    repo: Option<&'a str>,
    /// `"local"` for a dropped file; absent for catalog installs.
    source: Option<&'a str>,
    icon_url: Option<&'a str>,
}

impl Integrated {
    fn registry_entry(
        &self,
        id: &str,
        name: &str,
        version: &str,
        dest: &Path,
        origin: &Origin,
    ) -> Value {
        let mut entry = json!({
            "id": id,
            "name": name,
            "version": version,
            "path": dest.to_string_lossy(),
            "repo": origin.repo,
            "iconUrl": origin.icon_url,
            "iconFile": self.icon_file.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "desktopFile": self.desktop_path.to_string_lossy(),
            "runDir": self.run_dir.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "runStamp": self.run_stamp,
            "installedAt": now_secs(),
        });
        if let Some(s) = origin.source {
            entry["source"] = Value::String(s.to_string());
        }
        entry
    }
}

/// After a (re)install of `id`: delete what the previous entry owned and the
/// new one does not — the previous version's .AppImage and run dir. The
/// owner's box had fox-vs-chicken 0.55.0 sitting next to 0.57.0 because the
/// drag-and-drop reinstall never did this.
fn remove_superseded(previous: Option<&Value>, new_path: &Path, keep: &[&Path]) {
    let Some(prev) = previous else { return };
    if let Some(old) = prev.get("path").and_then(|v| v.as_str()) {
        let old = Path::new(old);
        if old != new_path && !keep.contains(&old) {
            let _ = fs::remove_file(old);
        }
    }
    if let Some(old_run) = prev.get("runDir").and_then(|v| v.as_str()) {
        let old_run = Path::new(old_run);
        // the run dir is keyed by id, so it is normally the same path (already
        // swapped in place); only a moved install dir leaves one behind
        if is_run_dir(old_run) && old_run != run_dir_for(new_path, id_of(prev)) {
            let _ = fs::remove_dir_all(old_run);
        }
    }
}

fn id_of(entry: &Value) -> &str {
    entry.get("id").and_then(|v| v.as_str()).unwrap_or("")
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

    let previous = registry::get_appimage(app, &id)?;
    emit_progress(app, &id, "integrating", 0, 0);
    let integrated = integrate(
        app,
        &id,
        &req.name,
        &version,
        &dest,
        req.icon_url.as_deref(),
        None,
    )
    .await?;

    let entry = integrated.registry_entry(
        &id,
        &req.name,
        &version,
        &dest,
        &Origin {
            repo: req.repo.as_deref(),
            source: None,
            icon_url: req.icon_url.as_deref(),
        },
    );
    registry::upsert_appimage(app, &id, entry.clone())?;
    remove_superseded(previous.as_ref(), &dest, &[]);
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
    let run_dir = entry
        .get("runDir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| {
            entry
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| run_dir_for(Path::new(p), &id))
        });
    if let Some(rd) = run_dir.filter(|p| is_run_dir(p)) {
        let _ = fs::remove_dir_all(&rd);
        if let Some(root) = rd.parent() {
            sweep_leftovers(root, &id);
            let _ = fs::remove_dir(root); // only succeeds once empty
        }
    }
    let r = registry::remove_appimage(&app, &id);
    registry::manifest_forget("appimages", &id);
    r
}

#[tauri::command]
pub async fn list_appimages(app: AppHandle) -> Result<Vec<Value>, String> {
    // Opening the list is a good moment to catch a tree that went stale (an
    // app that replaced its own .AppImage) — cheap when nothing changed.
    spawn_ensure_run_dirs(app.clone());
    registry::appimages(&app)
}

// ---------- run-dir migration ----------

/// Does `entry` need its tree (re)built? `recorded` is the stamp Komble last
/// extracted (or failed to extract) at, `run_dir` what it recorded, and
/// `run_dir_present` whether that tree is on disk now. A recorded refusal
/// (`runStamp` set, `runDir` null) is honoured until the file changes, so an
/// AppImage that cannot extract is not retried on every start.
pub(crate) fn needs_run_dir(
    recorded: Option<&str>,
    run_dir: Option<&str>,
    run_dir_present: bool,
    current: &str,
) -> bool {
    if recorded != Some(current) {
        return true;
    }
    run_dir.is_some() && !run_dir_present
}

static ENSURING: AtomicBool = AtomicBool::new(false);

fn spawn_ensure_run_dirs(app: AppHandle) {
    if ENSURING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        ensure_run_dirs_inner(&app).await;
        ENSURING.store(false, Ordering::SeqCst);
    });
}

/// Give every registered AppImage an extracted run dir, lazily and in the
/// background: installs from before this existed have none; a tree is rebuilt
/// when its .AppImage changed or the tree vanished. Each entry is
/// re-integrated with the same code an install uses, and the registry and
/// desktop entry are rewritten only after a successful extract. Idempotent:
/// a second pass over an up-to-date registry does no work.
pub fn ensure_run_dirs(app: AppHandle) {
    spawn_ensure_run_dirs(app);
}

async fn ensure_run_dirs_inner(app: &AppHandle) {
    let Ok(entries) = registry::appimages(app) else {
        return;
    };
    for entry in entries {
        let id = id_of(&entry).to_string();
        let Some(path) = entry.get("path").and_then(|v| v.as_str()) else {
            continue;
        };
        let appimage = PathBuf::from(path);
        let Some(current) = file_stamp(&appimage) else {
            continue; // file gone — not ours to fix here
        };
        let recorded = entry.get("runStamp").and_then(|v| v.as_str());
        let run_dir = entry.get("runDir").and_then(|v| v.as_str());
        let present = run_dir
            .map(|d| apprun_is_executable(Path::new(d)))
            .unwrap_or(false);
        if id.is_empty() || !needs_run_dir(recorded, run_dir, present, &current) {
            continue;
        }
        let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or(&id);
        let version = entry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("latest");
        let icon_url = entry.get("iconUrl").and_then(|v| v.as_str());
        let keep_icon = entry
            .get("iconFile")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        match integrate(app, &id, name, version, &appimage, icon_url, keep_icon).await {
            Ok(integrated) => {
                let mut updated = entry.clone();
                updated["desktopFile"] =
                    Value::String(integrated.desktop_path.to_string_lossy().into_owned());
                updated["iconFile"] = integrated
                    .icon_file
                    .map(|p| Value::String(p.to_string_lossy().into_owned()))
                    .unwrap_or(Value::Null);
                updated["runDir"] = integrated
                    .run_dir
                    .map(|p| Value::String(p.to_string_lossy().into_owned()))
                    .unwrap_or(Value::Null);
                updated["runStamp"] = integrated
                    .run_stamp
                    .map(Value::String)
                    .unwrap_or(Value::Null);
                if let Err(e) = registry::upsert_appimage(app, &id, updated) {
                    eprintln!("komble: run dir for {id}: registry write failed: {e}");
                }
            }
            Err(e) => eprintln!("komble: run dir for {id}: {e}"),
        }
    }
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

    let previous = registry::get_appimage(&app, &id)?;
    emit_progress(&app, &id, "integrating", 0, 0);
    let integrated = integrate(&app, &id, &name, &version, &dest, None, None).await?;

    let entry = integrated.registry_entry(
        &id,
        &name,
        &version,
        &dest,
        &Origin {
            repo: None,
            source: Some("local"),
            icon_url: None,
        },
    );
    registry::upsert_appimage(&app, &id, entry.clone())?;
    // never delete the file the user dropped, even if it was the old install
    remove_superseded(previous.as_ref(), &dest, &[&src]);
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

/// Download the newest release into the same directory and swap the tree in;
/// `install_inner` deletes the previous version's file and run dir.
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
    install_inner(&app, req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    static SEQ: AtomicU32 = AtomicU32::new(0);

    fn scratch(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        // under target/, not /tmp: the fake AppImages must be executable
        let d = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-scratch")
            .join(format!("{}-{tag}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn test_dirs(root: &Path) -> Dirs {
        Dirs {
            applications: root.join("applications"),
            icons_png: root.join("icons/256"),
            icons_svg: root.join("icons/svg"),
        }
    }

    /// A stand-in AppImage: a shell script that, asked to
    /// `--appimage-extract`, lays out `squashfs-root/` in the cwd the way the
    /// real runtime does (AppRun + .desktop + icon + .DirIcon symlink).
    fn fake_appimage(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    const EXTRACTS: &str = r#"
[ "$1" = "--appimage-extract" ] || exit 3
mkdir -p squashfs-root/usr/bin
printf '#!/bin/sh\nexec "$APPDIR/usr/bin/fake" "$@"\n' > squashfs-root/AppRun
chmod 755 squashfs-root/AppRun
printf '#!/bin/sh\necho fake\n' > squashfs-root/usr/bin/fake
chmod 755 squashfs-root/usr/bin/fake
printf '[Desktop Entry]\nType=Application\nName=Fake App\nComment=Pretends\nExec=fake %%U\nIcon=fake\nCategories=Network;\nMimeType=x-scheme-handler/fake;\n' > squashfs-root/fake.desktop
printf 'PNG' > squashfs-root/fake.png
ln -s fake.png squashfs-root/.DirIcon
"#;

    #[test]
    fn exec_args_are_quoted_per_spec() {
        assert_eq!(exec_arg("/a/b c"), "\"/a/b c\"");
        // the spec's four-backslashes-for-one rule; `%` is not a field code
        assert_eq!(exec_arg("x\"y"), "\"x\\\\\"y\"");
        assert_eq!(exec_arg("a\\b"), "\"a\\\\\\\\b\"");
        assert_eq!(exec_arg("100%"), "\"100%%\"");
        assert_eq!(exec_arg("$HOME`"), "\"\\\\$HOME\\\\`\"");
    }

    #[test]
    fn exec_line_points_at_apprun_with_appimage_env() {
        let t = ExecTarget::RunDir {
            appimage: PathBuf::from("/home/me/.local/share/appimages/chrome.AppImage"),
            run_dir: PathBuf::from("/home/me/.local/share/appimages/.komble-run/google-chrome"),
        };
        assert_eq!(
            t.exec_line(),
            "env \"APPIMAGE=/home/me/.local/share/appimages/chrome.AppImage\" \
             \"APPDIR=/home/me/.local/share/appimages/.komble-run/google-chrome\" \
             \"/home/me/.local/share/appimages/.komble-run/google-chrome/AppRun\" %U"
        );
        assert_eq!(
            t.try_exec().as_deref(),
            Some("/home/me/.local/share/appimages/.komble-run/google-chrome/AppRun")
        );
    }

    #[test]
    fn exec_line_falls_back_to_the_appimage() {
        let t = ExecTarget::AppImage(PathBuf::from("/x/My App.AppImage"));
        assert_eq!(t.exec_line(), "\"/x/My App.AppImage\" %U");
        assert_eq!(t.try_exec().as_deref(), Some("/x/My App.AppImage"));
        let odd = ExecTarget::AppImage(PathBuf::from("/x/back\\slash.AppImage"));
        assert_eq!(odd.try_exec(), None, "TryExec is a plain string value");
    }

    #[test]
    fn desktop_entry_takes_metadata_from_the_original() {
        let orig = "[Desktop Entry]\nName=Real Name\nComment=Hi\nIcon=realicon\nCategories=Game;\nMimeType=x/y;\n[Other]\nName=Nope\n";
        let t = ExecTarget::AppImage(PathBuf::from("/x/a.AppImage"));
        let d = build_desktop_entry(&DesktopSpec {
            id: "a",
            version: "1.0",
            original: orig,
            fallback_name: "Fallback",
            has_icon_file: false,
            target: &t,
        });
        assert!(d.contains("Name=Real Name\n"));
        assert!(d.contains("Icon=realicon\n"));
        assert!(d.contains("MimeType=x/y;\n"));
        assert!(d.contains("TryExec=/x/a.AppImage\nExec=\"/x/a.AppImage\" %U\n"));
        assert!(d.contains("X-Komble-Id=a\nX-AppImage-Version=1.0\n"));
        let d2 = build_desktop_entry(&DesktopSpec {
            id: "a",
            version: "1.0",
            original: "",
            fallback_name: "Fallback",
            has_icon_file: true,
            target: &t,
        });
        assert!(d2.contains("Name=Fallback\n"));
        assert!(d2.contains("Icon=komble-a\n"));
        assert!(d2.contains("Categories=Utility;\n"));
    }

    #[test]
    fn run_dir_lives_hidden_next_to_the_appimage() {
        let ai = Path::new("/home/me/.local/share/appimages/foo-1-2.AppImage");
        assert_eq!(
            run_dir_for(ai, "foo"),
            PathBuf::from("/home/me/.local/share/appimages/.komble-run/foo")
        );
        assert!(is_run_dir(&run_dir_for(ai, "foo")));
        assert!(!is_run_dir(Path::new("/home/me/.local/share/appimages")));
        assert!(!is_run_dir(Path::new("/home/me")));
        assert!(!is_run_dir(Path::new(".komble-run/foo")));
    }

    #[test]
    fn migration_decides_from_stamp_and_presence() {
        // pre-run-dir install: never extracted
        assert!(needs_run_dir(None, None, false, "10-1"));
        // extracted for this very file and the tree is there: nothing to do
        assert!(!needs_run_dir(
            Some("10-1"),
            Some("/r/.komble-run/a"),
            true,
            "10-1"
        ));
        // tree vanished
        assert!(needs_run_dir(
            Some("10-1"),
            Some("/r/.komble-run/a"),
            false,
            "10-1"
        ));
        // the .AppImage changed (self-update through $APPIMAGE, new version)
        assert!(needs_run_dir(
            Some("10-1"),
            Some("/r/.komble-run/a"),
            true,
            "11-2"
        ));
        // extraction was refused for this file: leave it alone until it changes
        assert!(!needs_run_dir(Some("10-1"), None, false, "10-1"));
        assert!(needs_run_dir(Some("10-1"), None, false, "12-9"));
    }

    #[test]
    fn integrate_keeps_the_tree_and_runs_apprun() {
        let root = scratch("keep");
        let install = root.join("appimages");
        fs::create_dir_all(&install).unwrap();
        let ai = fake_appimage(&install, "fake-app-1-0-0.AppImage", EXTRACTS);
        let d = test_dirs(&root);

        let staged = stage(&d, "fake-app", &ai);
        let run_dir = staged.run_dir.clone().expect("tree kept");
        assert_eq!(run_dir, install.join(".komble-run/fake-app"));
        assert!(apprun_is_executable(&run_dir));
        assert!(run_dir.join("fake.desktop").is_file());
        assert!(staged.original_desktop.contains("Name=Fake App"));
        assert_eq!(
            staged.icon_file.as_deref(),
            Some(root.join("icons/256/komble-fake-app.png").as_path())
        );
        // no work dir left behind, only the tree
        let names: Vec<String> = fs::read_dir(install.join(".komble-run"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["fake-app".to_string()]);

        let desktop =
            write_desktop_entry(&d, "fake-app", "Fake", "1.0.0", &ai, &staged, true).unwrap();
        let text = fs::read_to_string(&desktop).unwrap();
        let apprun = run_dir.join("AppRun").to_string_lossy().into_owned();
        assert!(text.contains(&format!(
            "Exec=env \"APPIMAGE={}\" \"APPDIR={}\" \"{apprun}\" %U\n",
            ai.display(),
            run_dir.display()
        )));
        assert!(text.contains(&format!("TryExec={apprun}\n")));
        assert!(text.contains("Name=Fake App\n"));
        assert!(text.contains("Icon=komble-fake-app\n"));
        assert!(text.contains("MimeType=x-scheme-handler/fake;\n"));

        // and the tree actually runs, with APPDIR as the Exec line sets it
        let out = std::process::Command::new(run_dir.join("AppRun"))
            .env("APPDIR", &run_dir)
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "fake");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn integrate_falls_back_when_extraction_is_refused() {
        let root = scratch("refuse");
        let install = root.join("appimages");
        fs::create_dir_all(&install).unwrap();
        let ai = fake_appimage(&install, "stubborn-1.AppImage", "exit 1");
        let d = test_dirs(&root);
        let staged = stage(&d, "stubborn", &ai);
        assert!(staged.run_dir.is_none());
        assert!(!install.join(".komble-run/stubborn").exists());
        assert!(
            fs::read_dir(install.join(".komble-run")).unwrap().count() == 0,
            "the work dir is cleaned up"
        );
        let desktop =
            write_desktop_entry(&d, "stubborn", "Stubborn", "1", &ai, &staged, false).unwrap();
        let text = fs::read_to_string(desktop).unwrap();
        assert!(text.contains(&format!("Exec=\"{}\" %U\n", ai.display())));
        assert!(text.contains("Icon=application-x-executable\n"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn integrate_falls_back_when_apprun_is_not_executable() {
        let root = scratch("noexec");
        let install = root.join("appimages");
        fs::create_dir_all(&install).unwrap();
        let ai = fake_appimage(
            &install,
            "odd-1.AppImage",
            "mkdir -p squashfs-root && : > squashfs-root/AppRun && chmod 644 squashfs-root/AppRun",
        );
        let staged = stage(&test_dirs(&root), "odd", &ai);
        assert!(staged.run_dir.is_none());
        assert!(!install.join(".komble-run/odd").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_second_extract_replaces_the_tree_in_place() {
        let root = scratch("swap");
        let install = root.join("appimages");
        fs::create_dir_all(&install).unwrap();
        let ai = fake_appimage(&install, "fake-1.AppImage", EXTRACTS);
        let run_root = run_root_for(&ai);
        let first = extract_run_tree(&ai, &run_root, "fake").unwrap();
        fs::write(first.join("marker-from-v1"), "x").unwrap();
        // a leftover from a crashed run must not survive either
        fs::create_dir_all(run_root.join(".extract-fake-999/squashfs-root")).unwrap();
        let second = extract_run_tree(&ai, &run_root, "fake").unwrap();
        assert_eq!(first, second);
        assert!(!second.join("marker-from-v1").exists(), "old tree is gone");
        assert!(apprun_is_executable(&second));
        let names: Vec<String> = fs::read_dir(&run_root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["fake".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn superseded_files_go_but_the_dropped_source_stays() {
        let root = scratch("supersede");
        let old = root.join("fox-0-55-0.AppImage");
        let new = root.join("fox-0-57-0.AppImage");
        fs::write(&old, "o").unwrap();
        fs::write(&new, "n").unwrap();
        let prev = json!({ "id": "fox", "path": old.to_string_lossy(), "runDir": run_dir_for(&old, "fox").to_string_lossy() });
        remove_superseded(Some(&prev), &new, &[]);
        assert!(!old.exists(), "the previous version's file is deleted");
        assert!(new.exists());

        // re-dropping the very file that is installed: it is kept
        fs::write(&old, "o").unwrap();
        remove_superseded(Some(&prev), &new, &[&old]);
        assert!(old.exists());

        // a run dir in another install dir is removed; the same one is not
        let other_root = root.join("elsewhere");
        let other_run = other_root.join(RUN_ROOT).join("fox");
        fs::create_dir_all(&other_run).unwrap();
        let prev2 = json!({ "id": "fox", "path": other_root.join("fox-1.AppImage").to_string_lossy(), "runDir": other_run.to_string_lossy() });
        remove_superseded(Some(&prev2), &new, &[]);
        assert!(!other_run.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn file_stamp_tracks_size_and_mtime() {
        let root = scratch("stamp");
        let f = root.join("a.AppImage");
        fs::write(&f, "abc").unwrap();
        let s1 = file_stamp(&f).unwrap();
        assert!(s1.starts_with("3-"));
        fs::write(&f, "abcdef").unwrap();
        let s2 = file_stamp(&f).unwrap();
        assert!(s2.starts_with("6-"));
        assert_ne!(s1, s2);
        assert_eq!(file_stamp(&root.join("missing.AppImage")), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_names_and_versions() {
        assert_eq!(
            parse_appimage_name("MyApp-1.2.3-x86_64"),
            ("MyApp".to_string(), "1.2.3".to_string())
        );
        assert_eq!(
            parse_appimage_name("fox-vs-chicken-0.57.0"),
            ("fox vs chicken".to_string(), "0.57.0".to_string())
        );
        assert_eq!(
            parse_appimage_name("tool"),
            ("tool".to_string(), "local".to_string())
        );
    }
}
