//! Arch repositories (pacman) + the AUR.
//!
//! Everything privileged goes through `pkexec`, and every command is built as an
//! argv vector — never a shell string — so there is no interpolation and no
//! metacharacter surface. Package names and file paths are validated before they
//! reach the helper, and the helper validates them again.
//!
//! Three things about Arch shape this module and are NOT stylistic choices:
//!
//! 1. **There are no partial upgrades.** `pacman -S <one-package>` on a system
//!    whose sync DB is newer than its installed set is how you break Arch. So
//!    there is deliberately no "upgrade this one package" command — the Updates
//!    view offers a full `-Syu` and nothing else.
//! 2. **`pacman -Sy` on its own is the same trap**: it desyncs the DB from what
//!    is installed and arms exactly that breakage. So we never run it. The update
//!    list comes from `checkupdates`, which syncs into a private temp DB and
//!    leaves the real one alone.
//! 3. **`makepkg` refuses to run as root**, so an AUR build cannot be a verb
//!    under pkexec like every other privileged action. It builds as the user and
//!    only the final `pacman -U` is privileged.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Command;

use crate::registry;
use crate::util::{estr, now_secs, which};

const HELPER: &str = "/usr/lib/komble/komble-helper";

fn helper_available() -> bool {
    std::path::Path::new(HELPER).exists()
}

/// Exposed for the Settings → System panel.
pub fn helper_installed() -> bool {
    helper_available()
}

/// Install a single repo package without the registry bookkeeping — used by the
/// one-click system fixes (e.g. fuse2 for AppImage launching).
pub async fn install_package_named(package: &str) -> Result<String, String> {
    if !valid_pkg_name(package) {
        return Err("invalid package name".into());
    }
    run_privileged(
        vec!["pacman", "-S", "--noconfirm", "--needed", "--", package],
        vec!["install-repo", package],
    )
    .await
}

// ── shapes the frontend binds to ────────────────────────────────────────────
// Deliberately identical in shape to what the Debian build returned, so the
// Svelte views are a rename rather than a rewrite. `section` carries the repo
// (core/extra/multilib/aur) — Arch has repos and groups, not Debian sections.

#[derive(Serialize)]
pub struct PkgUpdate {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub source: String, // "repo" | "aur"
}

#[derive(Serialize)]
pub struct PkgFileInfo {
    pub path: String,
    pub package: String,
    pub version: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct BrowseItem {
    pub name: String,
    pub summary: String,
    pub section: String,
    pub version: String,
    pub installed: bool,
}

#[derive(Serialize)]
pub struct BrowseResult {
    pub total: usize,
    pub items: Vec<BrowseItem>,
}

#[derive(Deserialize)]
struct AurPkg {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
}

#[derive(Deserialize)]
struct AurReply {
    results: Vec<AurPkg>,
}

// ── validation ──────────────────────────────────────────────────────────────
// Arch package names are more permissive than Debian's: alphanumerics plus
// @ . _ + -, and they may contain uppercase. They may not start with a hyphen
// or a dot. Copying Debian's lowercase-only rule here would silently reject
// perfectly ordinary packages.
fn valid_pkg_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 128
        && !n.starts_with('-')
        && !n.starts_with('.')
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | '-'))
}

fn validate_pkg_file(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err("package path must be absolute".into());
    }
    let p = p.canonicalize().map_err(|e| format!("{path}: {e}"))?;
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    if !name.contains(".pkg.tar") {
        return Err("not a pacman package (*.pkg.tar.zst)".into());
    }
    if !p.is_file() {
        return Err("not a file".into());
    }
    Ok(p)
}

// ── privileged execution ────────────────────────────────────────────────────
async fn run_privileged(direct: Vec<&str>, helper: Vec<&str>) -> Result<String, String> {
    let mut cmd = Command::new("pkexec");
    if helper_available() {
        cmd.arg(HELPER).args(&helper);
    } else {
        cmd.args(&direct);
    }
    // stable output for parsing regardless of the user's locale
    cmd.env("LANG", "C").env("LC_ALL", "C");
    let out = cmd.output().await.map_err(estr)?;
    let mut log = String::from_utf8_lossy(&out.stdout).to_string();
    log.push_str(&String::from_utf8_lossy(&out.stderr));

    invalidate_installed_cache();
    if out.status.success() {
        return Ok(log);
    }
    Err(match out.status.code() {
        Some(126) => "Authentication dialog was dismissed.".into(),
        Some(127) => "Not authorized (polkit refused).".into(),
        _ => {
            let tail: String = log.chars().rev().take(2000).collect::<String>().chars().rev().collect();
            format!("Command failed.\n{tail}")
        }
    })
}

// ── plain queries ───────────────────────────────────────────────────────────
async fn run_out(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(estr)?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

static INSTALLED_CACHE: Mutex<Option<(Instant, HashSet<String>)>> = Mutex::new(None);

fn invalidate_installed_cache() {
    if let Ok(mut g) = INSTALLED_CACHE.lock() {
        *g = None;
    }
}

async fn installed_set() -> HashSet<String> {
    if let Ok(g) = INSTALLED_CACHE.lock() {
        if let Some((at, set)) = g.as_ref() {
            if at.elapsed() < Duration::from_secs(60) {
                return set.clone();
            }
        }
    }
    let text = run_out("pacman", &["-Qq"]).await.unwrap_or_default();
    let set: HashSet<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    if let Ok(mut g) = INSTALLED_CACHE.lock() {
        *g = Some((Instant::now(), set.clone()));
    }
    set
}

async fn installed_version(pkg: &str) -> Option<String> {
    let text = run_out("pacman", &["-Q", pkg]).await.ok()?;
    text.split_whitespace().nth(1).map(|s| s.to_string())
}

// ── the available-package index ─────────────────────────────────────────────
struct Entry {
    name: String,
    summary: String,
    repo: String,
    version: String,
}

static INDEX: Mutex<Option<std::sync::Arc<Vec<Entry>>>> = Mutex::new(None);

pub fn invalidate_index() {
    if let Ok(mut g) = INDEX.lock() {
        *g = None;
    }
}

/// Built once and cached. `expac` reads the sync DBs directly and emits one line
/// per package, which is far cheaper than `pacman -Si` over ~14k packages; if it
/// is not installed we fall back to `pacman -Sl`, which costs us descriptions
/// but still gives a browsable list.
async fn build_index() -> Vec<Entry> {
    if which("expac") {
        let text = run_out("expac", &["-S", "%r\\t%n\\t%v\\t%d"]).await.unwrap_or_default();
        let mut out = Vec::with_capacity(16_000);
        let mut seen = HashSet::new();
        for line in text.lines() {
            let mut f = line.split('\t');
            let (repo, name, version, desc) = (f.next(), f.next(), f.next(), f.next());
            if let (Some(repo), Some(name), Some(version)) = (repo, name, version) {
                if name.is_empty() || !seen.insert(name.to_string()) {
                    continue;
                }
                out.push(Entry {
                    name: name.to_string(),
                    summary: desc.unwrap_or_default().to_string(),
                    repo: repo.to_string(),
                    version: version.to_string(),
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }

    let text = run_out("pacman", &["-Sl"]).await.unwrap_or_default();
    let mut out = Vec::with_capacity(16_000);
    let mut seen = HashSet::new();
    for line in text.lines() {
        let mut f = line.split_whitespace();
        if let (Some(repo), Some(name), Some(version)) = (f.next(), f.next(), f.next()) {
            if !seen.insert(name.to_string()) {
                continue;
            }
            out.push(Entry {
                name: name.to_string(),
                summary: String::new(),
                repo: repo.to_string(),
                version: version.to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

async fn index() -> std::sync::Arc<Vec<Entry>> {
    if let Ok(g) = INDEX.lock() {
        if let Some(v) = g.as_ref() {
            return v.clone();
        }
    }
    let built = std::sync::Arc::new(build_index().await);
    if let Ok(mut g) = INDEX.lock() {
        *g = Some(built.clone());
    }
    built
}

#[tauri::command]
pub async fn browse_packages(
    query: Option<String>,
    section: Option<String>,
    limit: Option<usize>,
) -> Result<BrowseResult, String> {
    let idx = index().await;
    let installed = installed_set().await;
    let q = query.unwrap_or_default().trim().to_lowercase();
    let repo = section.filter(|s| !s.trim().is_empty());

    let mut matches: Vec<&Entry> = idx
        .iter()
        .filter(|e| {
            if let Some(r) = &repo {
                if &e.repo != r {
                    return false;
                }
            }
            // names may contain uppercase on Arch, so lowercase BOTH sides —
            // the Debian build compared a raw name against a lowered needle
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.summary.to_lowercase().contains(&q)
        })
        .collect();

    let total = matches.len();
    if !q.is_empty() {
        matches.sort_by_key(|e| {
            let n = e.name.to_lowercase();
            if n == q {
                0
            } else if n.starts_with(&q) {
                1
            } else if n.contains(&q) {
                2
            } else {
                3
            }
        });
    }
    let limit = limit.unwrap_or(2000).min(5000);
    let items = matches
        .into_iter()
        .take(limit)
        .map(|e| BrowseItem {
            name: e.name.clone(),
            summary: e.summary.clone(),
            section: e.repo.clone(),
            version: e.version.clone(),
            installed: installed.contains(&e.name),
        })
        .collect();
    Ok(BrowseResult { total, items })
}

/// Repos rather than Debian sections — core / extra / multilib / whatever the
/// user has enabled, each with a package count.
#[tauri::command]
pub async fn package_repos() -> Result<Vec<(String, usize)>, String> {
    let idx = index().await;
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for e in idx.iter() {
        *counts.entry(e.repo.clone()).or_insert(0) += 1;
    }
    Ok(counts.into_iter().collect())
}

#[tauri::command]
pub async fn package_info(package: String) -> Result<Value, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    // -Si is the repo copy; fall back to -Qi so AUR/foreign packages still show
    let mut text = run_out("pacman", &["-Si", &package]).await.unwrap_or_default();
    if text.trim().is_empty() {
        text = run_out("pacman", &["-Qi", &package]).await.unwrap_or_default();
    }
    if text.trim().is_empty() {
        return Err(format!("{package}: not found"));
    }

    let get = |key: &str| -> String {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim() == key {
                    return v.trim().to_string();
                }
            }
        }
        String::new()
    };

    let version = get("Version");
    let repo = get("Repository");
    let url = get("URL");
    let desc = get("Description");
    let size = get("Installed Size");

    // "Installed Size : 12.34 MiB" → KiB, so the UI's existing field still works
    let size_kb = {
        let mut it = size.split_whitespace();
        let n: f64 = it.next().unwrap_or("0").parse().unwrap_or(0.0);
        match it.next().unwrap_or("") {
            "MiB" => (n * 1024.0) as u64,
            "GiB" => (n * 1024.0 * 1024.0) as u64,
            "B" => (n / 1024.0) as u64,
            _ => n as u64,
        }
    };

    Ok(json!({
        "package": package,
        "version": version,
        "installedVersion": installed_version(&package).await,
        "section": if repo.is_empty() { "aur".to_string() } else { repo },
        "homepage": if url.is_empty() { Value::Null } else { Value::String(url) },
        "summary": desc.clone(),
        "description": desc,
        "installedSizeKb": size_kb,
    }))
}

// ── install / remove ────────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_package(app: AppHandle, package: String) -> Result<String, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    let log = run_privileged(
        vec!["pacman", "-S", "--noconfirm", "--needed", "--", &package],
        vec!["install-repo", &package],
    )
    .await?;

    let version = installed_version(&package).await.unwrap_or_default();
    let _ = registry::upsert_package(
        &app,
        &package,
        json!({
            "package": package,
            "version": version,
            "description": "",
            "source": "repo",
            "installedAt": now_secs(),
        }),
    );
    invalidate_index();
    Ok(log)
}

#[tauri::command]
pub async fn remove_package(app: AppHandle, package: String) -> Result<String, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    // -Rs takes unused dependencies with it, which is what a user removing an
    // app means. It never removes something another package still needs.
    let log = run_privileged(
        vec!["pacman", "-Rs", "--noconfirm", "--", &package],
        vec!["remove", &package],
    )
    .await?;
    let _ = registry::remove_package(&app, &package);
    invalidate_index();
    Ok(log)
}

#[tauri::command]
pub async fn package_file_info(path: String) -> Result<PkgFileInfo, String> {
    let p = validate_pkg_file(&path)?;
    let ps = p.to_string_lossy().to_string();
    let text = run_out("pacman", &["-Qip", &ps]).await?;
    let mut info = PkgFileInfo {
        path: ps,
        package: String::new(),
        version: String::new(),
        description: String::new(),
    };
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            match k.trim() {
                "Name" => info.package = v.trim().to_string(),
                "Version" => info.version = v.trim().to_string(),
                "Description" => info.description = v.trim().to_string(),
                _ => {}
            }
        }
    }
    if info.package.is_empty() {
        return Err("could not read package metadata".into());
    }
    Ok(info)
}

#[tauri::command]
pub async fn install_package_file(app: AppHandle, path: String) -> Result<String, String> {
    let p = validate_pkg_file(&path)?;
    let ps = p.to_string_lossy().to_string();
    let info = package_file_info(ps.clone()).await?;
    let log = run_privileged(
        vec!["pacman", "-U", "--noconfirm", "--", &ps],
        vec!["install-file", &ps],
    )
    .await?;
    let _ = registry::upsert_package(
        &app,
        &info.package,
        json!({
            "package": info.package,
            "version": info.version,
            "description": info.description,
            "source": ps,
            "installedAt": now_secs(),
        }),
    );
    invalidate_index();
    Ok(log)
}

#[tauri::command]
pub async fn list_tracked_packages(app: AppHandle) -> Result<Vec<Value>, String> {
    registry::packages(&app)
}

// ── updates ─────────────────────────────────────────────────────────────────
/// `checkupdates` (pacman-contrib) syncs into a private temp DB, so it can report
/// updates WITHOUT touching the real one. Running `pacman -Sy` to get the same
/// answer would leave the system in a partial-upgrade state, which is the single
/// most common way to break an Arch install.
pub async fn upgradable_inner() -> Result<Vec<PkgUpdate>, String> {
    let mut out = Vec::new();
    if which("checkupdates") {
        let text = run_out("checkupdates", &[]).await.unwrap_or_default();
        for line in text.lines() {
            // "name oldver -> newver"
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 4 && f[2] == "->" {
                out.push(PkgUpdate {
                    name: f[0].to_string(),
                    current: f[1].to_string(),
                    latest: f[3].to_string(),
                    source: "repo".into(),
                });
            }
        }
    }
    out.extend(aur_updates().await.unwrap_or_default());
    Ok(out)
}

#[tauri::command]
pub async fn list_upgradable() -> Result<Vec<PkgUpdate>, String> {
    upgradable_inner().await
}

/// Foreign packages (`-Qm`) are the AUR ones; ask the AUR RPC for their current
/// versions in a single batched request.
async fn aur_updates() -> Result<Vec<PkgUpdate>, String> {
    let text = run_out("pacman", &["-Qm"]).await.unwrap_or_default();
    let mut have: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let mut f = line.split_whitespace();
        if let (Some(n), Some(v)) = (f.next(), f.next()) {
            have.push((n.to_string(), v.to_string()));
        }
    }
    if have.is_empty() {
        return Ok(vec![]);
    }
    let mut url = "https://aur.archlinux.org/rpc/v5/info".to_string();
    for (i, (n, _)) in have.iter().enumerate() {
        url.push_str(if i == 0 { "?arg[]=" } else { "&arg[]=" });
        url.push_str(&urlencoding(n));
    }
    let reply: AurReply = crate::util::client()
        .get(&url)
        .send()
        .await
        .map_err(estr)?
        .json()
        .await
        .map_err(estr)?;

    let mut out = Vec::new();
    for p in reply.results {
        let latest = p.version.unwrap_or_default();
        if let Some((_, cur)) = have.iter().find(|(n, _)| *n == p.name) {
            if !latest.is_empty() && &latest != cur {
                out.push(PkgUpdate {
                    name: p.name,
                    current: cur.clone(),
                    latest,
                    source: "aur".into(),
                });
            }
        }
    }
    Ok(out)
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A full `-Syu` and nothing else. There is deliberately no per-package upgrade:
/// on a rolling release that is a partial upgrade, and partial upgrades are
/// unsupported by design — the Updates view says so rather than offering a
/// button that quietly breaks the system.
#[tauri::command]
pub async fn system_upgrade() -> Result<String, String> {
    let log = run_privileged(
        vec!["pacman", "-Syu", "--noconfirm"],
        vec!["sysupgrade"],
    )
    .await?;
    invalidate_index();
    Ok(log)
}

/// "Refresh lists" costs nothing and touches nothing: checkupdates already works
/// against its own database copy, so all we do is drop our in-memory index.
#[tauri::command]
pub async fn refresh_lists() -> Result<String, String> {
    invalidate_index();
    invalidate_installed_cache();
    Ok(String::new())
}

// ── AUR ─────────────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn aur_search(query: String) -> Result<Vec<BrowseItem>, String> {
    let q = query.trim();
    if q.len() < 2 {
        return Ok(vec![]);
    }
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/search/{}?by=name-desc",
        urlencoding(q)
    );
    let reply: AurReply = crate::util::client()
        .get(&url)
        .send()
        .await
        .map_err(estr)?
        .json()
        .await
        .map_err(estr)?;
    let installed = installed_set().await;
    let mut items: Vec<BrowseItem> = reply
        .results
        .into_iter()
        .map(|p| BrowseItem {
            installed: installed.contains(&p.name),
            name: p.name,
            summary: p.description.unwrap_or_default(),
            section: "aur".into(),
            version: p.version.unwrap_or_default(),
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.truncate(200);
    Ok(items)
}

/// The PKGBUILD, verbatim, for the user to read BEFORE anything runs.
///
/// This is not a nicety. A PKGBUILD is a shell script that executes with the
/// user's privileges at build time and can do anything that user can. Every AUR
/// install in this app is gated on showing this text first — that is the whole
/// security model, and it is why the AUR path is a review step rather than a
/// one-click button.
#[tauri::command]
pub async fn aur_pkgbuild(package: String) -> Result<String, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        urlencoding(&package)
    );
    let res = crate::util::client().get(&url).send().await.map_err(estr)?;
    if !res.status().is_success() {
        return Err(format!("no PKGBUILD for {package} (HTTP {})", res.status()));
    }
    res.text().await.map_err(estr)
}

/// Clone, build as the USER, then install the built artifact with pkexec.
///
/// makepkg refuses to run as root, so this cannot be a privileged verb like the
/// others. PACMAN_AUTH=pkexec is how makepkg is told to elevate for its own
/// dependency installs — without it, it shells out to sudo and hangs forever
/// waiting for a password on a terminal that does not exist.
#[tauri::command]
pub async fn aur_install(app: AppHandle, package: String) -> Result<String, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    if !which("makepkg") {
        return Err("makepkg not found — install base-devel".into());
    }
    if !which("git") {
        return Err("git not found — install git".into());
    }

    let base = app
        .path()
        .app_cache_dir()
        .map_err(estr)?
        .join("aur")
        .join(&package);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).map_err(estr)?;

    let _ = app.emit("install-progress", json!({ "id": package, "stage": "clone" }));
    let clone = Command::new("git")
        .args(["clone", "--depth", "1"])
        .arg(format!("https://aur.archlinux.org/{package}.git"))
        .arg(&base)
        .output()
        .await
        .map_err(estr)?;
    if !clone.status.success() {
        return Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&clone.stderr)
        ));
    }

    let _ = app.emit("install-progress", json!({ "id": package, "stage": "build" }));
    let build = Command::new("makepkg")
        .args(["--noconfirm", "--syncdeps", "--clean"])
        .current_dir(&base)
        .env("PACMAN_AUTH", "pkexec")
        .output()
        .await
        .map_err(estr)?;
    if !build.status.success() {
        let err = String::from_utf8_lossy(&build.stderr);
        let tail: String = err.chars().rev().take(2000).collect::<String>().chars().rev().collect();
        return Err(format!("makepkg failed:\n{tail}"));
    }

    // whatever it produced — version and arch are the PKGBUILD's business
    let built = std::fs::read_dir(&base)
        .map_err(estr)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.contains(".pkg.tar"))
                .unwrap_or(false)
        })
        .ok_or_else(|| "makepkg produced no package file".to_string())?;

    let _ = app.emit("install-progress", json!({ "id": package, "stage": "install" }));
    install_package_file(app.clone(), built.to_string_lossy().to_string()).await
}
