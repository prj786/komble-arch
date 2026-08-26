//! AppImage catalog + GitHub release resolution, with disk caching.
//!
//! The catalog is the AM database (github.com/ivan-hc/AM, served as JSON from
//! portable-linux-apps.github.io). The old appimage.github.io feed was largely
//! abandoned — full of dead projects and entries with no fetchable release —
//! while AM is actively curated and every listed app has an install script we
//! can mine for the download source (`resolve_am_app`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

use crate::util::{client, estr, slug};

const AM_APPS_URL: &str = "https://portable-linux-apps.github.io/apps.json";
const AM_SCRIPT_URL: &str = "https://raw.githubusercontent.com/ivan-hc/AM/main/programs/x86_64/";
const AM_PAGE_URL: &str = "https://portable-linux-apps.github.io/apps/";
// The Pages site stopped serving /icons/*.png (404), but the same files are
// still in the site's repo — fetch them raw.
const AM_ICON_URL: &str =
    "https://raw.githubusercontent.com/Portable-Linux-Apps/Portable-Linux-Apps.github.io/main/icons/";
const FEED_TTL: u64 = 24 * 3600;
const RELEASE_TTL: u64 = 6 * 3600;

// ---------- models ----------

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub authors: Vec<Author>,
    pub license: Option<String>,
    pub github: Option<String>,
    pub download: Option<String>,
    pub icon: Option<String>,
    pub screenshots: Vec<String>,
    /// "am" — items whose download source is resolved lazily via resolve_am_app
    #[serde(default)]
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Author {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub published_at: Option<String>,
    pub assets: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub name: String,
    pub url: String,
    pub size: u64,
}

// ---------- raw feed shapes ----------

/// apps.json changed shape in 2026-08: it used to be an array of
/// `{packageName, description, icon, arch}`, now it's a map of
/// `id → {description, archs}` (no icon field — see AM_ICON_URL).
type AmFeed = BTreeMap<String, AmApp>;

#[derive(Deserialize)]
struct AmApp {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    archs: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

// ---------- caching helpers ----------

pub fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_cache_dir().map_err(estr)?;
    fs::create_dir_all(&dir).map_err(estr)?;
    Ok(dir)
}

fn fresh(path: &Path, ttl: u64) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|d| d.as_secs() < ttl)
        .unwrap_or(false)
}

// ---------- catalog ----------

/// "qbittorrent" → "Qbittorrent", "3d-puzzles" → "3d Puzzles". The AM list has
/// no display names, only package slugs.
fn prettify(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn map_items(raw: AmFeed) -> Vec<CatalogItem> {
    let mut out = Vec::with_capacity(raw.len());
    // BTreeMap iteration → the catalog arrives alphabetized by id.
    for (id, item) in raw {
        if id.trim().is_empty() {
            continue;
        }
        // Only apps AM actually builds/links for x86_64 are installable here.
        if !item
            .archs
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|a| a == "x86_64")
        {
            continue;
        }
        out.push(CatalogItem {
            name: prettify(&id),
            description: item.description,
            categories: vec![],
            authors: vec![],
            license: None,
            github: None,
            download: Some(format!("{AM_PAGE_URL}{id}.html")),
            // Constructed, not listed in the feed — a missing file just 404s
            // and the card falls back to its glyph (AppCard's on:error).
            icon: Some(format!("{AM_ICON_URL}{id}.png")),
            screenshots: vec![],
            source: "am".into(),
            id,
        });
    }
    out
}

#[tauri::command]
pub async fn fetch_catalog(
    app: AppHandle,
    force: Option<bool>,
) -> Result<Vec<CatalogItem>, String> {
    let cache = cache_dir(&app)?.join("am-apps.json");
    if !force.unwrap_or(false) && fresh(&cache, FEED_TTL) {
        if let Ok(text) = fs::read_to_string(&cache) {
            // A cache in the pre-2026-08 array shape fails this parse and
            // falls through to a refetch, which overwrites it.
            if let Ok(raw) = serde_json::from_str::<AmFeed>(&text) {
                return Ok(map_items(raw));
            }
        }
    }
    let fetched = async {
        client()
            .get(AM_APPS_URL)
            .send()
            .await
            .map_err(|e| format!("Could not reach the AM app database: {e}"))?
            .error_for_status()
            .map_err(estr)?
            .text()
            .await
            .map_err(estr)
    }
    .await;
    let text = match fetched {
        Ok(t) => t,
        // Offline: a stale cache beats an empty Discover page.
        Err(e) => fs::read_to_string(&cache).map_err(|_| e)?,
    };
    let raw: AmFeed = serde_json::from_str(&text).map_err(|e| format!("Bad app database: {e}"))?;
    let _ = fs::write(&cache, &text);
    Ok(map_items(raw))
}

// ---------- AM install-script resolution ----------

/// What `resolve_am_app` figured out: when the app lives on GitHub we hand back
/// the repo URL too, so the install can record it and the Updates view can
/// track new releases.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmResolved {
    pub github: Option<String>,
    pub release: ReleaseInfo,
}

fn valid_am_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// Crude "1.2.3" out of an AppImage file name, for direct (non-GitHub) downloads.
fn version_from_name(name: &str) -> Option<String> {
    name.split(['-', '_'])
        .map(|t| t.trim_start_matches(['v', 'V']))
        .find(|t| {
            !t.is_empty()
                && t.chars().next().is_some_and(|c| c.is_ascii_digit())
                && t.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .map(String::from)
}

/// Every AM app has a plain-sh install script. Nearly all of them declare
/// `SITE="owner/repo"` (GitHub) — resolve those through the Releases API like
/// any other GitHub app. The rest scrape a download page in their `version=`
/// line; do the same scrape here and take the best-arch .AppImage link.
#[tauri::command]
pub async fn resolve_am_app(
    app: AppHandle,
    id: String,
    token: Option<String>,
) -> Result<AmResolved, String> {
    if !valid_am_id(&id) {
        return Err("invalid app id".into());
    }
    let script = client()
        .get(format!("{AM_SCRIPT_URL}{id}"))
        .send()
        .await
        .map_err(estr)?
        .error_for_status()
        .map_err(|_| format!("{id}: no install recipe found (the app may have been renamed)."))?
        .text()
        .await
        .map_err(estr)?;

    let site = script.lines().find_map(|l| {
        l.trim()
            .strip_prefix("SITE=")
            .map(|v| v.trim_matches('"').trim_matches('\'').to_string())
    });
    if let Some(site) = site {
        let looks_like_repo = !site.starts_with("http")
            && site.matches('/').count() == 1
            && !site.contains(' ')
            && !site.contains('$')
            && site != "REPLACETHIS";
        if looks_like_repo {
            let github = format!("https://github.com/{site}");
            let release = resolve_release(app, github.clone(), token).await?;
            return Ok(AmResolved {
                github: Some(github),
                release,
            });
        }
    }

    // Non-GitHub app: its version= line curls a page and greps for *.AppImage —
    // replicate that. `version=$(curl -Ls https://... | ...)`
    let page = script
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("version="))
        .find_map(|l| {
            let idx = l.find("http")?;
            let rest = &l[idx..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '|')
                .unwrap_or(rest.len());
            Some(rest[..end].to_string())
        })
        .ok_or_else(|| format!("{id}: could not determine a download source."))?;
    let body = client()
        .get(&page)
        .send()
        .await
        .map_err(estr)?
        .text()
        .await
        .map_err(estr)?;
    let mut urls: Vec<&str> = body
        .split(['"', '\'', '<', '>', '(', ')', ',', ' ', '\n', '\r', '\t'])
        .filter(|t| {
            t.starts_with("http") && t.len() < 500 && t.to_lowercase().ends_with(".appimage")
        })
        .collect();
    urls.sort_by_key(|u| arch_score(u));
    urls.dedup();
    let url = urls
        .first()
        .ok_or_else(|| format!("{id}: no .AppImage download found on the project page."))?
        .to_string();
    let file = url.rsplit('/').next().unwrap_or("app.AppImage").to_string();
    let version = version_from_name(&file).unwrap_or_else(|| "latest".into());
    Ok(AmResolved {
        github: None,
        release: ReleaseInfo {
            tag: version.clone(),
            version,
            published_at: None,
            assets: vec![AssetInfo {
                name: file,
                url,
                size: 0,
            }],
        },
    })
}

// ---------- GitHub releases ----------

fn parse_repo(github_url: &str) -> Option<(String, String)> {
    let rest = github_url.split("github.com/").nth(1)?;
    let mut parts = rest.split('/').filter(|s| !s.is_empty());
    let owner = parts.next()?.to_string();
    let repo = parts
        .next()?
        .trim_end_matches(".git")
        .split(&['#', '?'][..])
        .next()?
        .to_string();
    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

async fn gh_get(url: &str, token: &Option<String>) -> Result<reqwest::Response, String> {
    let mut req = client()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token.as_deref().filter(|t| !t.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", t.trim()));
    }
    req.send().await.map_err(estr)
}

fn arch_score(name: &str) -> u8 {
    let n = name.to_lowercase();
    if n.contains("x86_64") || n.contains("amd64") || n.contains("x64") {
        0
    } else if n.contains("aarch64")
        || n.contains("arm64")
        || n.contains("armhf")
        || n.contains("i386")
        || n.contains("i686")
        || n.contains("riscv")
    {
        2
    } else {
        1
    }
}

fn to_release_info(rel: GhRelease) -> Result<ReleaseInfo, String> {
    let mut assets: Vec<AssetInfo> = rel
        .assets
        .into_iter()
        .filter(|a| {
            let n = a.name.to_lowercase();
            n.ends_with(".appimage")
        })
        .map(|a| AssetInfo {
            name: a.name,
            url: a.browser_download_url,
            size: a.size,
        })
        .collect();
    if assets.is_empty() {
        return Err("The latest release has no .AppImage asset.".into());
    }
    assets.sort_by_key(|a| arch_score(&a.name));
    let version = rel
        .tag_name
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();
    Ok(ReleaseInfo {
        tag: rel.tag_name,
        version,
        published_at: rel.published_at,
        assets,
    })
}

fn has_appimage(rel: &GhRelease) -> bool {
    rel.assets
        .iter()
        .any(|a| a.name.to_lowercase().ends_with(".appimage"))
}

#[tauri::command]
pub async fn resolve_release(
    app: AppHandle,
    github_url: String,
    token: Option<String>,
) -> Result<ReleaseInfo, String> {
    let (owner, repo) =
        parse_repo(&github_url).ok_or_else(|| format!("Not a GitHub repo URL: {github_url}"))?;

    let rel_dir = cache_dir(&app)?.join("releases");
    fs::create_dir_all(&rel_dir).map_err(estr)?;
    let cache = rel_dir.join(format!("{}--{}.json", slug(&owner), slug(&repo)));
    if fresh(&cache, RELEASE_TTL) {
        if let Ok(text) = fs::read_to_string(&cache) {
            if let Ok(info) = serde_json::from_str::<ReleaseInfo>(&text) {
                return Ok(info);
            }
        }
    }

    let base = format!("https://api.github.com/repos/{owner}/{repo}/releases");
    let resp = gh_get(&format!("{base}/latest"), &token).await?;
    let info = match resp.status().as_u16() {
        200 => to_release_info(resp.json::<GhRelease>().await.map_err(estr)?)?,
        403 | 429 => return Err(
            "GitHub API rate limit reached. Add a personal access token in Settings to raise it."
                .into(),
        ),
        404 => {
            // No "latest" release (or only prereleases): scan the list.
            let resp = gh_get(&format!("{base}?per_page=20"), &token).await?;
            match resp.status().as_u16() {
                403 | 429 => {
                    return Err(
                        "GitHub API rate limit reached. Add a personal access token in Settings to raise it."
                            .into(),
                    )
                }
                200 => {
                    // List is newest-first: prefer the newest stable release
                    // with an AppImage asset, else the newest prerelease.
                    let mut list: Vec<GhRelease> = resp.json().await.map_err(estr)?;
                    let idx = list
                        .iter()
                        .position(|r| !r.prerelease && has_appimage(r))
                        .or_else(|| list.iter().position(has_appimage));
                    match idx {
                        Some(i) => to_release_info(list.swap_remove(i))?,
                        None => return Err("No release with an .AppImage asset found.".into()),
                    }
                }
                s => return Err(format!("GitHub API error: HTTP {s}")),
            }
        }
        s => return Err(format!("GitHub API error: HTTP {s}")),
    };

    let _ = fs::write(&cache, serde_json::to_string(&info).unwrap_or_default());
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The 2026-08 apps.json shape: a map of id → {description, archs}.
    #[test]
    fn parses_am_map_feed() {
        let text = r#"{
            "0ad": {"description": "RTS game", "archs": ["x86_64"]},
            "arm-only": {"description": "not for us", "archs": ["aarch64"]},
            "no-archs": {"description": "skipped"}
        }"#;
        let raw: AmFeed = serde_json::from_str(text).unwrap();
        let items = map_items(raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "0ad");
        assert_eq!(items[0].description.as_deref(), Some("RTS game"));
        assert_eq!(
            items[0].icon.as_deref(),
            Some(concat!(
                "https://raw.githubusercontent.com/Portable-Linux-Apps/",
                "Portable-Linux-Apps.github.io/main/icons/0ad.png"
            ))
        );
    }
}
