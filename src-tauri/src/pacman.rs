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
use std::sync::atomic::{AtomicBool, Ordering};
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
    // every privileged action changes what is pending — tell the DE bar's
    // update indicator to re-count (fire-and-forget, no-op outside ewe)
    crate::de::poke_updates();
    if out.status.success() {
        return Ok(log);
    }
    Err(match out.status.code() {
        Some(126) => "Authentication dialog was dismissed.".into(),
        Some(127) => "Not authorized (polkit refused).".into(),
        _ => {
            let tail: String = log
                .chars()
                .rev()
                .take(2000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
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
    let set: HashSet<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if let Ok(mut g) = INSTALLED_CACHE.lock() {
        *g = Some((Instant::now(), set.clone()));
    }
    set
}

pub(crate) async fn installed_version(pkg: &str) -> Option<String> {
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
        let text = run_out("expac", &["-S", "%r\\t%n\\t%v\\t%d"])
            .await
            .unwrap_or_default();
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
    let mut items: Vec<BrowseItem> = matches
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

    // First-party ewe apps get a branded card at the top of a matching search
    // (section "ewe"), and the frontend routes their install through
    // install_first_party rather than pacman -S.
    //
    // They USED to be invisible to the index above — release-only, never in a
    // sync DB. That stopped being true once [ewe] started carrying them, so
    // without the dedup below a search for "komble" returned the app twice:
    // once from the sync DB, once injected. The branded card wins and the
    // sync-DB row is dropped.
    let mut total = total;
    if !q.is_empty() && repo.is_none() {
        for (name, _, summary) in crate::first_party::DISCOVER.iter().rev() {
            if name.contains(&q) || summary.to_lowercase().contains(&q) {
                if let Some(dup) = items.iter().position(|i| i.name == *name) {
                    items.remove(dup);
                    total = total.saturating_sub(1);
                }
                let ver = installed_version(name).await.unwrap_or_default();
                items.insert(
                    0,
                    BrowseItem {
                        name: name.to_string(),
                        summary: summary.to_string(),
                        section: "ewe".into(),
                        version: ver,
                        installed: installed.contains(*name),
                    },
                );
                total += 1;
            }
        }
    }
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
    let mut text = run_out("pacman", &["-Si", &package])
        .await
        .unwrap_or_default();
    if text.trim().is_empty() {
        text = run_out("pacman", &["-Qi", &package])
            .await
            .unwrap_or_default();
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
    registry::manifest_forget("packages", &package);
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

/// A package file the user picked themselves: not from a repo, not from the
/// AUR — but the manifest only knows those two kinds plus first-party, and a
/// restore verifies against the repos before falling back to the AUR, so
/// "repo" is the honest default for a hand-picked file.
#[tauri::command]
pub async fn install_package_file(app: AppHandle, path: String) -> Result<String, String> {
    install_package_file_as(app, path, "repo").await
}

/// `pacman -U` a package file and record it in the manifest as `source`
/// (`repo` | `aur` | `first-party`) — never the file path (see
/// registry::classify_source). The path is kept in `file` for reference.
pub(crate) async fn install_package_file_as(
    app: AppHandle,
    path: String,
    source: &str,
) -> Result<String, String> {
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
            "source": registry::classify_source(source),
            "file": ps,
            "installedAt": now_secs(),
        }),
    );
    invalidate_index();
    Ok(log)
}

/// Several repo packages in ONE transaction (one polkit prompt, one
/// dependency resolution): what a restore of "everything from the backup"
/// needs. Same helper verb as a single install — `install-repo` takes a list.
#[tauri::command]
pub async fn install_packages(app: AppHandle, packages: Vec<String>) -> Result<String, String> {
    if packages.is_empty() {
        return Err("no packages given".into());
    }
    for p in &packages {
        if !valid_pkg_name(p) {
            return Err(format!("invalid package name: {p}"));
        }
    }
    let names: Vec<&str> = packages.iter().map(String::as_str).collect();
    let mut direct = vec!["pacman", "-S", "--noconfirm", "--needed", "--"];
    direct.extend(names.iter().copied());
    let mut helper = vec!["install-repo"];
    helper.extend(names.iter().copied());
    let log = run_privileged(direct, helper).await?;

    for package in &packages {
        let version = installed_version(package).await.unwrap_or_default();
        let _ = registry::upsert_package(
            &app,
            package,
            json!({
                "package": package,
                "version": version,
                "description": "",
                "source": "repo",
                "installedAt": now_secs(),
            }),
        );
    }
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
        // Give checkupdates a PRIVATE sync-DB dir. The stock default
        // (/tmp/checkup-db-$UID) is shared with every other checkupdates on
        // the system — the DE bar's periodic check, a terminal run — and two
        // at once collide on that DB's lock: checkupdates exits 1 with empty
        // output, which used to read as "0 updates" and blank the badge.
        // Exit codes: 0 = updates listed, 2 = none, 1 = error. Errors get one
        // retry (a concurrent pacman sync, a mirror hiccup) and then surface,
        // instead of masquerading as an up-to-date system.
        let db = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(std::env::temp_dir)
            .join("komble/checkup-db");
        let mut text = String::new();
        let mut err = String::new();
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
            let r = Command::new("checkupdates")
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .env("CHECKUPDATES_DB", &db)
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(estr)?;
            match r.status.code() {
                Some(0) | Some(2) => {
                    text = String::from_utf8_lossy(&r.stdout).to_string();
                    err.clear();
                    break;
                }
                _ => {
                    let tail = String::from_utf8_lossy(&r.stderr);
                    err = tail
                        .lines()
                        .last()
                        .unwrap_or("checkupdates failed")
                        .to_string();
                }
            }
        }
        if !err.is_empty() {
            return Err(format!("Could not check for repo updates: {err}"));
        }
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

/// Packages pacman is told to leave alone (`IgnorePkg` in pacman.conf). The
/// bar's paru count honours these natively; counting them here made the two
/// numbers disagree on any machine with a pin.
fn ignored_pkgs() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    if let Ok(text) = std::fs::read_to_string("/etc/pacman.conf") {
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("IgnorePkg") {
                if let Some(v) = rest.split_once('=').map(|(_, v)| v) {
                    set.extend(v.split_whitespace().map(String::from));
                }
            }
        }
    }
    set
}

/// pacman's own version comparison — epochs, pkgrels, and locally-rebuilt
/// packages judged exactly like paru judges them. Plain `latest != cur`
/// counted a package as "updatable" when the LOCAL copy was the newer one.
async fn is_newer(latest: &str, cur: &str) -> bool {
    match run_out("vercmp", &[latest, cur]).await {
        Ok(o) => o.trim().parse::<i32>().map(|n| n > 0).unwrap_or(false),
        Err(_) => latest != cur,
    }
}

/// Foreign packages (`-Qm`) are the AUR ones; ask the AUR RPC for their current
/// versions in a single batched request.
async fn aur_updates() -> Result<Vec<PkgUpdate>, String> {
    let text = run_out("pacman", &["-Qm"]).await.unwrap_or_default();
    let ignored = ignored_pkgs();
    let mut have: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let mut f = line.split_whitespace();
        if let (Some(n), Some(v)) = (f.next(), f.next()) {
            if !ignored.contains(n) {
                have.push((n.to_string(), v.to_string()));
            }
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
            if !latest.is_empty() && is_newer(&latest, cur).await {
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

/// One upgrade at a time, process-wide. Two overlapping flows (e.g. "Update
/// all" and "Upgrade system" clicked in succession) used to run two makepkg
/// pipelines side by side — wasted bandwidth at best, colliding `pacman -U`
/// transactions at worst. The guard also drives the DE bar's "updating" state
/// for its whole lifetime, not just while pacman holds db.lck.
static UPGRADE_RUNNING: AtomicBool = AtomicBool::new(false);

struct UpgradeGuard;
impl UpgradeGuard {
    fn acquire() -> Result<Self, String> {
        if UPGRADE_RUNNING.swap(true, Ordering::SeqCst) {
            return Err("An upgrade is already running — let it finish first.".into());
        }
        Ok(UpgradeGuard)
    }
}
impl Drop for UpgradeGuard {
    fn drop(&mut self) {
        UPGRADE_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// A full `-Syu` and nothing else. There is deliberately no per-package upgrade:
/// on a rolling release that is a partial upgrade, and partial upgrades are
/// unsupported by design — the Updates view says so rather than offering a
/// button that quietly breaks the system.
#[tauri::command]
pub async fn system_upgrade() -> Result<String, String> {
    let _guard = UpgradeGuard::acquire()?;
    crate::de::poke_working_now(true).await;
    let r = run_privileged(vec!["pacman", "-Syu", "--noconfirm"], vec!["sysupgrade"]).await;
    invalidate_index();
    crate::de::poke_working_now(false).await;
    r
}

/// Rebuild every outdated AUR package. `pacman -Syu` never touches foreign
/// packages, so without this the AUR rows survive "Upgrade system" forever —
/// exactly the "old packages are still there" bug. Each package goes through
/// the same clone → makepkg → pacman -U pipeline as an install (the frontend
/// hears the same install-progress events); failures are collected so one
/// broken PKGBUILD doesn't strand the rest.
///
/// No PKGBUILD review gate here, deliberately: the gate exists so nothing the
/// user never approved gets to run, and these packages were each reviewed at
/// install time. Re-reviewing every diff on every rebuild is what paru's
/// default is, and nobody reads those either — the meaningful consent was at
/// install.
#[tauri::command]
pub async fn aur_upgrade(app: AppHandle) -> Result<String, String> {
    let _guard = UpgradeGuard::acquire()?;
    let pending = aur_updates().await.unwrap_or_default();
    if pending.is_empty() {
        return Ok(String::new());
    }
    crate::de::poke_working_now(true).await;
    let mut log = String::new();
    let mut failed: Vec<String> = Vec::new();
    for u in pending {
        // re-assert per package: the shell's crash-safety timeout would clear
        // the "updating" glyph mid-way through a long multi-package run
        crate::de::poke_working(true);
        match aur_install(app.clone(), u.name.clone(), None).await {
            Ok(l) => {
                log.push_str(&l);
                log.push('\n');
            }
            Err(e) => failed.push(format!("{}: {e}", u.name)),
        }
    }
    crate::de::poke_working_now(false).await;
    if failed.is_empty() {
        Ok(log)
    } else {
        Err(format!(
            "Some AUR packages failed to rebuild:\n{}",
            failed.join("\n")
        ))
    }
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

/// One GET with a single retry on a transport error (the AUR front end
/// resets idle connections now and then; a second attempt on a fresh
/// connection is what a browser would do). HTTP error statuses are NOT
/// retried — those are answers, not failures.
async fn aur_get(url: &str) -> Result<reqwest::Response, String> {
    let client = crate::util::client();
    match client.get(url).send().await {
        Ok(r) => Ok(r),
        Err(first) => {
            tokio::time::sleep(Duration::from_millis(400)).await;
            client
                .get(url)
                .send()
                .await
                .map_err(|second| format!("{first} (retry: {second})"))
        }
    }
}

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
    let reply: AurReply = aur_get(&url).await?.json().await.map_err(estr)?;
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
    let res = aur_get(&url).await?;
    if !res.status().is_success() {
        return Err(format!("no PKGBUILD for {package} (HTTP {})", res.status()));
    }
    res.text().await.map_err(estr)
}

// ── .SRCINFO: the machine-readable half of a PKGBUILD ───────────────────────
//
// makepkg verifies every `source` that ships a detached signature against the
// PKGBUILD's `validpgpkeys`, and refuses to build when a key is not in the
// USER's gpg keyring. A fresh install has an empty keyring, so every signed
// package (1password, many -bin packages) failed with "One or more PGP
// signatures could not be verified" — a global bug, not a per-package one.
// The keys are declared in .SRCINFO, which the AUR generates from the
// PKGBUILD and which every AUR clone carries; reading it (not the PKGBUILD)
// means no shell evaluation of untrusted text.

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AurMeta {
    pub validpgpkeys: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    /// at least one source has a `.sig`/`.asc` companion — makepkg WILL verify
    pub signed_sources: bool,
}

/// A PGP fingerprint (40 hex) or long key id (16 hex). Anything else never
/// reaches a gpg argv.
fn valid_fingerprint(k: &str) -> bool {
    (16..=40).contains(&k.len()) && k.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_srcinfo(text: &str) -> AurMeta {
    let mut m = AurMeta::default();
    for line in text.lines() {
        let Some((k, v)) = line.trim().split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        // split packages carry per-package keys as `depends_x86_64` etc.;
        // the architecture suffix does not change what we need
        let key = k.split('_').next().unwrap_or(k);
        match key {
            "validpgpkeys" => {
                let up = v.to_ascii_uppercase();
                if valid_fingerprint(&up) && !m.validpgpkeys.contains(&up) {
                    m.validpgpkeys.push(up);
                }
            }
            "depends" => m.depends.push(v.to_string()),
            "makedepends" => m.makedepends.push(v.to_string()),
            "source" => {
                let file = v.split("::").next().unwrap_or(v);
                if file.ends_with(".sig") || file.ends_with(".asc") || file.ends_with(".sign") {
                    m.signed_sources = true;
                }
            }
            _ => {}
        }
    }
    m
}

/// The package's .SRCINFO from the AUR — for the review card, BEFORE the
/// clone exists, so the user sees "signed by key X" next to the PKGBUILD.
#[tauri::command]
pub async fn aur_srcinfo(package: String) -> Result<AurMeta, String> {
    if !valid_pkg_name(&package) {
        return Err("invalid package name".into());
    }
    aur_srcinfo_remote(&package).await
}

async fn aur_srcinfo_remote(package: &str) -> Result<AurMeta, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        urlencoding(package)
    );
    let res = aur_get(&url).await?;
    if !res.status().is_success() {
        return Err(format!("no .SRCINFO for {package} (HTTP {})", res.status()));
    }
    Ok(parse_srcinfo(&res.text().await.map_err(estr)?))
}

/// After the clone: the checkout's own .SRCINFO is what makepkg will verify
/// against, so prefer it; the AUR copy is the fallback for a clone that
/// somehow lacks one.
async fn aur_srcinfo_for_checkout(base: &std::path::Path, package: &str) -> AurMeta {
    if let Ok(text) = std::fs::read_to_string(base.join(".SRCINFO")) {
        return parse_srcinfo(&text);
    }
    aur_srcinfo_remote(package).await.unwrap_or_default()
}

async fn gpg_has_key(fpr: &str) -> bool {
    Command::new("gpg")
        .args(["--batch", "--list-keys", fpr])
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Fetch the PKGBUILD's signing keys into the USER's keyring, the same way a
/// person would before `makepkg` (and the same fallback order ewe-repo's CI
/// uses). Nothing here is privileged: gpg creates ~/.gnupg itself and talks
/// to the keyservers through dirmngr. Each attempt is bounded — a keyserver
/// that blackholes must not hang the install forever.
async fn import_pgp_keys(app: &AppHandle, package: &str, keys: &[String]) -> Result<(), String> {
    const ATTEMPTS: [&[&str]; 3] = [
        &["--keyserver", "hkps://keyserver.ubuntu.com"],
        &["--keyserver", "hkps://keys.openpgp.org"],
        &[], // dirmngr's configured default
    ];
    if !which("gpg") {
        return Err(format!(
            "{package}'s sources are PGP-signed but gpg is not installed — install gnupg and retry."
        ));
    }
    let _ = app.emit(
        "install-progress",
        json!({ "id": package, "stage": "keys" }),
    );
    for fpr in keys {
        if !valid_fingerprint(fpr) {
            return Err(format!("{package}: malformed validpgpkeys entry {fpr:?}"));
        }
        if gpg_has_key(fpr).await {
            continue;
        }
        let mut last_err = String::new();
        let mut imported = false;
        for extra in ATTEMPTS {
            let mut cmd = Command::new("gpg");
            cmd.arg("--batch")
                .args(extra)
                .args(["--recv-keys", fpr])
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .stdin(Stdio::null())
                .kill_on_drop(true);
            match tokio::time::timeout(Duration::from_secs(30), cmd.output()).await {
                Ok(Ok(out)) if out.status.success() => {
                    imported = true;
                    break;
                }
                Ok(Ok(out)) => {
                    last_err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                }
                Ok(Err(e)) => last_err = e.to_string(),
                Err(_) => last_err = "keyserver did not answer within 30 s".into(),
            }
        }
        // `--recv-keys` can exit 0 without importing (keyserver returned
        // nothing) — trust the keyring, not the exit code
        if !imported || !gpg_has_key(fpr).await {
            return Err(format!(
                "{package}'s sources are signed by PGP key {fpr} and it could not be fetched \
                 from a keyserver (is the network up? is dirmngr blocked?).\n\
                 Import it yourself, then retry:\n  gpg --recv-keys {fpr}\n{last_err}"
            )
            .trim_end()
            .to_string());
        }
    }
    Ok(())
}

/// Clone, build as the USER, then install the built artifact with pkexec.
///
/// makepkg refuses to run as root, so this cannot be a privileged verb like the
/// others. PACMAN_AUTH=pkexec is how makepkg is told to elevate for its own
/// dependency installs — without it, it shells out to sudo and hangs forever
/// waiting for a password on a terminal that does not exist.
///
/// `skip_pgp_check` is the explicit, per-install, off-by-default override for
/// a package whose key genuinely cannot be fetched. It is never the default:
/// the signature is the one integrity check upstream ships.
#[tauri::command]
pub async fn aur_install(
    app: AppHandle,
    package: String,
    skip_pgp_check: Option<bool>,
) -> Result<String, String> {
    let r = aur_install_inner(
        app.clone(),
        package.clone(),
        skip_pgp_check.unwrap_or(false),
    )
    .await;
    // ALWAYS clear the progress entry — stage events ("clone"/"build") have no
    // terminal marker of their own, and a lingering entry pins "…" on cards
    let _ = app.emit(
        "install-progress",
        json!({ "id": package, "phase": "done" }),
    );
    r
}

/// Last ~2000 chars of a log — the part with the error in it.
fn tail(s: &str) -> String {
    s.chars()
        .rev()
        .take(2000)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

async fn aur_install_inner(
    app: AppHandle,
    package: String,
    skip_pgp_check: bool,
) -> Result<String, String> {
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

    let _ = app.emit(
        "install-progress",
        json!({ "id": package, "stage": "clone" }),
    );
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

    // the signing keys makepkg is about to verify against, from the checkout
    let meta = aur_srcinfo_for_checkout(&base, &package).await;
    if !meta.validpgpkeys.is_empty() && !skip_pgp_check {
        import_pgp_keys(&app, &package, &meta.validpgpkeys).await?;
    }

    let _ = app.emit(
        "install-progress",
        json!({ "id": package, "stage": "build" }),
    );
    let mut args = vec![
        "--noconfirm",
        "--syncdeps",
        "--needed",
        "--clean",
        "--noprogressbar",
    ];
    if skip_pgp_check {
        args.push("--skippgpcheck");
    }
    let build = Command::new("makepkg")
        .args(&args)
        .current_dir(&base)
        .env("PACMAN_AUTH", "pkexec")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(estr)?;
    // makepkg's own narration goes to stdout and the errors to stderr; a
    // build() failure without the stdout half is a blank "makepkg failed"
    let mut log = String::from_utf8_lossy(&build.stdout).to_string();
    log.push_str(&String::from_utf8_lossy(&build.stderr));
    // the whole log, so the UI can keep it on screen after the toast is gone
    let _ = app.emit(
        "install-log",
        json!({ "id": package, "ok": build.status.success(), "log": log }),
    );
    if !build.status.success() {
        return Err(format!("makepkg failed:\n{}", tail(&log)));
    }

    // whatever it produced — version and arch are the PKGBUILD's business.
    // The stock makepkg.conf enables `debug`, so a package that strips
    // binaries ALSO drops a <pkg>-debug-*.pkg.tar.zst beside the real one;
    // never install that by accident. Split packages: prefer the one that
    // carries the requested name.
    let mut built: Vec<PathBuf> = std::fs::read_dir(&base)
        .map_err(estr)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.contains(".pkg.tar") && !n.contains("-debug-"))
                .unwrap_or(false)
        })
        .collect();
    built.sort();
    let prefix = format!("{package}-");
    let built = built
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.starts_with(&prefix))
                .unwrap_or(false)
        })
        .or_else(|| built.first())
        .cloned()
        .ok_or_else(|| "makepkg produced no package file".to_string())?;

    let _ = app.emit(
        "install-progress",
        json!({ "id": package, "stage": "install" }),
    );
    install_package_file_as(app.clone(), built.to_string_lossy().to_string(), "aur").await
}
