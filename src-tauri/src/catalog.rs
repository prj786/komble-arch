//! AppImageHub catalog (feed.json) + GitHub release resolution, with disk caching.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

use crate::util::{client, estr, slug};

const FEED_URL: &str = "https://appimage.github.io/feed.json";
const DB_URL: &str = "https://appimage.github.io/database/";
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

#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    // The feed contains nulls *inside* arrays too, so every element is Option.
    #[serde(default)]
    categories: Option<Vec<Option<String>>>,
    #[serde(default)]
    authors: Option<Vec<Option<RawAuthor>>>,
    #[serde(default)]
    license: Option<serde_json::Value>,
    #[serde(default)]
    links: Option<Vec<Option<RawLink>>>,
    #[serde(default)]
    icons: Option<Vec<Option<String>>>,
    #[serde(default)]
    screenshots: Option<Vec<Option<String>>>,
}

#[derive(Deserialize)]
struct RawAuthor {
    name: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct RawLink {
    #[serde(rename = "type")]
    kind: Option<String>,
    url: Option<String>,
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

fn db_url(rel: &str) -> String {
    if rel.starts_with("http://") || rel.starts_with("https://") {
        rel.to_string()
    } else {
        format!("{DB_URL}{rel}")
    }
}

fn map_items(raw: RawFeed) -> Vec<CatalogItem> {
    let mut out = Vec::with_capacity(raw.items.len());
    for item in raw.items {
        let Some(name) = item.name.filter(|n| !n.trim().is_empty()) else {
            continue;
        };
        let mut github = None;
        let mut download = None;
        for link in item.links.unwrap_or_default().into_iter().flatten() {
            match (link.kind.as_deref(), link.url) {
                (Some("GitHub"), Some(url)) if !url.is_empty() => {
                    github = Some(if url.starts_with("http") {
                        url
                    } else {
                        format!("https://github.com/{url}")
                    })
                }
                (Some("Download"), Some(url)) if !url.is_empty() => download = Some(url),
                _ => {}
            }
        }
        out.push(CatalogItem {
            id: slug(&name),
            name,
            description: item.description,
            categories: item
                .categories
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .collect(),
            authors: item
                .authors
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .map(|a| Author { name: a.name, url: a.url })
                .collect(),
            license: item.license.and_then(|l| match l {
                serde_json::Value::String(s) if !s.is_empty() => Some(s),
                serde_json::Value::Object(o) => o
                    .get("name")
                    .or_else(|| o.get("id"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                _ => None,
            }),
            github,
            download,
            icon: item
                .icons
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .next()
                .map(|i| db_url(&i)),
            screenshots: item
                .screenshots
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .map(|s| db_url(&s))
                .collect(),
        });
    }
    out
}

#[tauri::command]
pub async fn fetch_catalog(app: AppHandle, force: Option<bool>) -> Result<Vec<CatalogItem>, String> {
    let cache = cache_dir(&app)?.join("feed.json");
    if !force.unwrap_or(false) && fresh(&cache, FEED_TTL) {
        if let Ok(text) = fs::read_to_string(&cache) {
            if let Ok(raw) = serde_json::from_str::<RawFeed>(&text) {
                return Ok(map_items(raw));
            }
        }
    }
    let text = client()
        .get(FEED_URL)
        .send()
        .await
        .map_err(|e| format!("Could not reach AppImageHub: {e}"))?
        .error_for_status()
        .map_err(estr)?
        .text()
        .await
        .map_err(estr)?;
    let raw: RawFeed = serde_json::from_str(&text).map_err(|e| format!("Bad feed: {e}"))?;
    let _ = fs::write(&cache, &text);
    Ok(map_items(raw))
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
    let version = rel.tag_name.trim_start_matches('v').trim_start_matches('V').to_string();
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
        403 | 429 => {
            return Err(
                "GitHub API rate limit reached. Add a personal access token in Settings to raise it."
                    .into(),
            )
        }
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
