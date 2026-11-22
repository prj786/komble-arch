//! eep apps, through the `eep` CLI (ewe's app manager: apps prebuilt from
//! the Nix binary cache, one card per app in `~/Applications`). The CLI is
//! the single implementation — it plans, fetches, writes the card and the
//! launcher entry; this file only runs it and hands its JSON to the UI.
//!
//! Every eep command prints ONE JSON object on stdout and exits 0; a failure
//! is `{"ok":false,"error":"…"}`. So the reply decides, never the exit
//! status. Without `eep` on PATH the source does not exist: nothing is
//! spawned and the UI shows no eep anything. No root, no pkexec: eep works
//! as the user (Nix's daemon does the store).
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const BIN: &str = "eep";
const NO_ANSWER: &str = "eep gave no answer Komble could read";

/// Is eep on PATH? (The one check that decides whether the source shows.)
pub fn present() -> bool {
    crate::util::which(BIN)
}

/// An app id as eep's catalog has them (`firefox`, `libreoffice`). Checked
/// before it reaches the argv; the first character is never `-`, so an id
/// cannot become a flag.
fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 80
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(format!("“{id}” is not an eep app"))
    }
}

/// eep's reply: the one JSON object on stdout (the last line that is not
/// empty — eep prints it last). `ok: false` is a failure whatever the exit
/// status was; output that is not a JSON object is one too.
fn reply(stdout: &str) -> Result<Value, String> {
    let last = stdout.lines().rev().map(str::trim).find(|l| !l.is_empty());
    let v = last
        .and_then(|l| serde_json::from_str::<Value>(l).ok())
        .or_else(|| serde_json::from_str::<Value>(stdout.trim()).ok())
        .filter(Value::is_object)
        .ok_or_else(|| NO_ANSWER.to_string())?;
    if v.get("ok") == Some(&Value::Bool(false)) {
        let why = string_of(v.get("error"));
        return Err(if why.is_empty() {
            "eep failed without saying why".into()
        } else {
            why
        });
    }
    Ok(v)
}

/// The last line eep wrote on stderr: what it said when it died before
/// printing its JSON (a panic, a missing library).
fn with_stderr(r: Result<Value, String>, stderr: &str) -> Result<Value, String> {
    let last = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty());
    r.map_err(|e| match last {
        Some(last) if e == NO_ANSWER => format!("{e}: {last}"),
        _ => e,
    })
}

/// Run one eep command and read its reply.
async fn run(args: &[&str]) -> Result<Value, String> {
    run_bin(BIN, args).await
}

async fn run_bin(bin: &str, args: &[&str]) -> Result<Value, String> {
    let out = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("couldn’t start eep: {e}"))?;
    with_stderr(
        reply(&String::from_utf8_lossy(&out.stdout)),
        &String::from_utf8_lossy(&out.stderr),
    )
}

/// Pieces fetched so far, read off Nix's own lines on eep's stderr: "these
/// 38 paths will be fetched (…)" (or "this path will be fetched") first,
/// then one "copying path '…' from '…'" per piece, patched ones included.
/// eep has no progress JSON (yet); when Nix words it differently the bar
/// simply stays indeterminate.
#[derive(Debug, Default, PartialEq, Eq)]
struct Fetching {
    total: u64,
    done: u64,
}

impl Fetching {
    /// Feed one stderr line; true when the numbers moved.
    fn feed(&mut self, line: &str) -> bool {
        let l = line.trim();
        if l.starts_with("this path will be fetched") {
            self.total += 1;
            return true;
        }
        if let Some(rest) = l.strip_prefix("these ") {
            let n = rest
                .split_whitespace()
                .next()
                .and_then(|n| n.parse::<u64>().ok());
            if let (Some(n), true) = (n, rest.contains("will be fetched")) {
                self.total += n;
                return true;
            }
            return false;
        }
        if l.starts_with("copying path ") && self.total > 0 {
            self.done = (self.done + 1).min(self.total);
            return true;
        }
        false
    }
}

/// `install-progress` for each app of a running command — the same event
/// the AppImage downloads send, under the id the UI gives eep apps.
fn emit(app: &AppHandle, ids: &[String], phase: &str, f: &Fetching) {
    for id in ids {
        let _ = app.emit(
            "install-progress",
            json!({ "id": format!("eep:{id}"), "phase": phase, "downloaded": f.done, "total": f.total }),
        );
    }
}

/// Run an install or update with its fetch progress streamed to the UI.
async fn run_watched(app: &AppHandle, ids: &[String], args: &[&str]) -> Result<Value, String> {
    let (app, ids) = (app.clone(), ids.to_vec());
    run_streamed(BIN, args, move |f| emit(&app, &ids, "downloading", f)).await
}

/// Run one command, reading its stderr as it comes: `on_fetch` hears every
/// move of the fetch, and the last line is kept for a reply-less death.
async fn run_streamed<F>(bin: &str, args: &[&str], on_fetch: F) -> Result<Value, String>
where
    F: Fn(&Fetching) + Send + 'static,
{
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("couldn’t start eep: {e}"))?;
    let stderr = child.stderr.take();
    let watcher = tokio::spawn(async move {
        let mut last = String::new();
        let Some(stderr) = stderr else {
            return last;
        };
        let mut f = Fetching::default();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if f.feed(&line) {
                on_fetch(&f);
            }
            if !line.trim().is_empty() {
                last = line;
            }
        }
        last
    });
    let out = child.wait_with_output().await.map_err(crate::util::estr)?;
    let last = watcher.await.unwrap_or_default();
    with_stderr(reply(&String::from_utf8_lossy(&out.stdout)), &last)
}

const NOT_HERE: &str = "eep is not installed on this machine";

/// install / update / remove, as the user. Holds a `Working` guard: closing
/// the window mid-download lets it finish (lib.rs). `done` always follows,
/// so a card never stays "Installing…" after a failure.
async fn change(app: &AppHandle, verb: &str, ids: Vec<String>) -> Result<Value, String> {
    if !present() {
        return Err(NOT_HERE.into());
    }
    for id in &ids {
        check_id(id)?;
    }
    if ids.is_empty() && verb != "update" {
        return Err("no app to change".into());
    }
    let _working = crate::util::Working::begin();
    let mut args = vec![verb];
    args.extend(ids.iter().map(String::as_str));
    let r = if verb == "remove" {
        run(&args).await
    } else {
        run_watched(app, &ids, &args).await
    };
    emit(app, &ids, "done", &Fetching::default());
    r.map(|v| changed_of(&v))
}

// ── the replies, normalised ─────────────────────────────────────────────────
// eep's JSON is the contract (docs/CARD.md in the eep repo); every field but
// `id` is optional here so a field an eep does not send yet never hides the
// whole list.

fn string_of(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn u64_of(v: Option<&Value>) -> u64 {
    v.and_then(|n| n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)))
        .unwrap_or(0)
}

fn bool_of(v: Option<&Value>) -> bool {
    v.and_then(Value::as_bool).unwrap_or(false)
}

fn items<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| check_id(&string_of(a.get("id"))).is_ok())
}

/// One app of `eep available`.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EepApp {
    pub id: String,
    pub name: String,
    /// gui | cli | tool
    pub kind: String,
    pub version: String,
    /// one line (the catalog's descriptions carry hard line breaks)
    pub description: String,
    /// what the page shows: see [`icon_of`]; "" = the initial on its tile
    pub icon: String,
    /// http(s) only; "" = none
    pub homepage: String,
    /// bytes to download for a first install, and on disk afterwards
    pub download: u64,
    pub disk: u64,
    /// why it cannot be installed from this release; "" = it can
    pub unavailable: String,
    /// the installed version; None = not installed
    pub installed: Option<String>,
    pub update: bool,
}

fn apps_of(v: &Value) -> Vec<EepApp> {
    items(v, "apps")
        .map(|a| {
            let id = string_of(a.get("id"));
            let name = string_of(a.get("name"));
            EepApp {
                name: if name.is_empty() { id.clone() } else { name },
                kind: string_of(a.get("kind")),
                version: string_of(a.get("version")),
                description: string_of(a.get("description"))
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                icon: icon_of(a),
                homepage: Some(string_of(a.get("homepage")))
                    .filter(|h| h.starts_with("https://") || h.starts_with("http://"))
                    .unwrap_or_default(),
                download: u64_of(a.get("download")),
                disk: u64_of(a.get("disk")),
                unavailable: match a.get("unavailable") {
                    Some(Value::String(s)) => s.trim().to_string(),
                    Some(Value::Bool(true)) => "not in this release".into(),
                    _ => String::new(),
                },
                installed: match a.get("installed") {
                    Some(Value::String(s)) => Some(s.trim().to_string()),
                    Some(Value::Bool(true)) => Some(String::new()),
                    _ => None,
                },
                update: bool_of(a.get("update")),
                id,
            }
        })
        .collect()
}

/// The icon the page shows: eep's cached copy (`icon_file`, written on `eep
/// index update`) as a data: URL; else `icon_url` when it is https (the
/// page loads https and data: images only); else none.
fn icon_of(a: &Value) -> String {
    let file = string_of(a.get("icon_file"));
    if let Some(url) = Some(Path::new(&file))
        .filter(|p| p.is_absolute())
        .and_then(icon_data_url)
    {
        return url;
    }
    Some(string_of(a.get("icon_url")))
        .filter(|u| u.starts_with("https://"))
        .unwrap_or_default()
}

/// Bigger than any app icon (eep's cards cap theirs at 96 KB).
const ICON_MAX: u64 = 512 * 1024;

/// A local PNG or SVG as a data: URL, cached by path and mtime: the list is
/// read again after every install, and each icon is encoded once.
fn icon_data_url(path: &Path) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (SystemTime, String)>>> = OnceLock::new();
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > ICON_MAX {
        return None;
    }
    let mtime = meta.modified().ok()?;
    let cache = CACHE.get_or_init(Default::default);
    let hit = cache.lock().ok().and_then(|c| {
        c.get(path)
            .filter(|(t, _)| *t == mtime)
            .map(|(_, url)| url.clone())
    });
    if hit.is_some() {
        return hit;
    }
    let url = format!("data:{mime};base64,{}", base64(&std::fs::read(path).ok()?));
    if let Ok(mut c) = cache.lock() {
        c.insert(path.to_path_buf(), (mtime, url.clone()));
    }
    Some(url)
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() {
                ABC[((n >> (18 - 6 * i)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// What install / update / remove did, whatever the shape: one app's
/// fields at the top level next to the totals (install or remove of one),
/// a list under `apps` (several), or under `updated` (update).
fn changed_of(v: &Value) -> Value {
    let one = check_id(&string_of(v.get("id"))).is_ok().then_some(v);
    let apps: Vec<Value> = one
        .into_iter()
        .chain(items(v, "apps"))
        .chain(items(v, "updated"))
        .map(|a| json!({ "id": string_of(a.get("id")), "version": string_of(a.get("version")) }))
        .collect();
    json!({
        "download": u64_of(v.get("download")),
        "downloadFull": u64_of(v.get("download_full")),
        "seconds": v.get("seconds").and_then(Value::as_f64).unwrap_or(0.0),
        "apps": apps,
    })
}

/// One installed app of `eep list` (its card).
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EepInstalled {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub version: String,
}

fn installed_of(v: &Value) -> Vec<EepInstalled> {
    items(v, "apps")
        .map(|a| {
            let id = string_of(a.get("id"));
            let name = string_of(a.get("name"));
            EepInstalled {
                name: if name.is_empty() { id.clone() } else { name },
                kind: string_of(a.get("kind")),
                version: string_of(a.get("version")),
                id,
            }
        })
        .collect()
}

/// One pending update of `eep updates`.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EepUpdate {
    pub id: String,
    pub name: String,
    pub from: String,
    pub to: String,
    /// same version, rebuilt against newer libraries
    pub rebuild_only: bool,
    /// bytes with patches, and without them
    pub download: u64,
    pub download_full: u64,
}

fn updates_of(v: &Value) -> Value {
    let updates: Vec<EepUpdate> = items(v, "updates")
        .map(|u| {
            let id = string_of(u.get("id"));
            let name = string_of(u.get("name"));
            EepUpdate {
                name: if name.is_empty() { id.clone() } else { name },
                from: string_of(u.get("from")),
                to: string_of(u.get("to")),
                rebuild_only: bool_of(u.get("rebuild_only")),
                download: u64_of(u.get("download")),
                download_full: u64_of(u.get("download_full")),
                id,
            }
        })
        .collect();
    // `--count` sends only the number; the full reply has both
    let count = v
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(updates.len() as u64);
    json!({
        "available": true,
        "count": count,
        "download": u64_of(v.get("download")),
        "downloadFull": u64_of(v.get("download_full")),
        "updates": updates,
    })
}

/// `eep doctor`: its own `ready` verdict, and when it is not ready, what
/// stops an install — Nix not reachable, no app list yet, no graphics driver.
fn doctor_of(v: &Value) -> Value {
    let mut problems = Vec::new();
    let nix = string_of(v.pointer("/nix/error"));
    if !nix.is_empty() {
        problems.push(format!("Nix isn’t reachable: {nix}"));
    }
    if v.get("index").is_none_or(Value::is_null) {
        problems.push("No app list yet. It comes with the next check for updates.".into());
    }
    if v.pointer("/driver/ok") == Some(&Value::Bool(false)) {
        problems.push("No graphics driver for eep apps yet (eep driver install).".into());
    }
    // an eep from before `ready` existed: the verdict is the problems
    let ready = v
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(problems.is_empty());
    if ready {
        problems.clear();
    } else if problems.is_empty() {
        problems.push("eep isn’t set up yet: eep doctor says what is missing.".into());
    }
    json!({ "available": true, "ready": ready, "problems": problems })
}

// ── commands ────────────────────────────────────────────────────────────────

/// What eep can install, with each app's state here. `refresh` asks eep's
/// server for a newer list first (`eep index update`: a conditional GET).
/// Never an Err while eep is here — a failure travels as `error`, so the
/// UI can tell "no eep" (`available: false`) from "eep could not answer".
#[tauri::command]
pub async fn eep_available(refresh: bool) -> Value {
    if !present() {
        return json!({ "available": false });
    }
    let warning = if refresh {
        run(&["index", "update"]).await.err()
    } else {
        None
    };
    match run(&["available"]).await {
        Ok(v) => json!({
            "available": true,
            "release": string_of(v.get("release")),
            "apps": apps_of(&v),
            "warning": warning,
        }),
        Err(e) => json!({ "available": true, "apps": [], "error": e }),
    }
}

/// The installed eep apps — their cards, read locally (no index, no network).
#[tauri::command]
pub async fn eep_list() -> Result<Vec<EepInstalled>, String> {
    if !present() {
        return Ok(Vec::new());
    }
    Ok(installed_of(&run(&["list"]).await?))
}

#[tauri::command]
pub async fn eep_install(app: AppHandle, ids: Vec<String>) -> Result<Value, String> {
    change(&app, "install", ids).await
}

#[tauri::command]
pub async fn eep_remove(app: AppHandle, ids: Vec<String>) -> Result<Value, String> {
    change(&app, "remove", ids).await
}

/// Update these apps; none named = every app with an update.
#[tauri::command]
pub async fn eep_update(app: AppHandle, ids: Vec<String>) -> Result<Value, String> {
    change(&app, "update", ids).await
}

/// Pending updates. `count`: only the number (`--count`, no Nix call — the
/// sidebar badge). `refresh`: a newer app list first; a failure there is a
/// `warning`, and the updates of the list eep already has still come back.
#[tauri::command]
pub async fn eep_updates(count: bool, refresh: bool) -> Result<Value, String> {
    if !present() {
        return Ok(json!({ "available": false, "count": 0, "updates": [] }));
    }
    let warning = if refresh {
        run(&["index", "update"]).await.err()
    } else {
        None
    };
    let args: &[&str] = if count {
        &["updates", "--count"]
    } else {
        &["updates"]
    };
    let mut v = updates_of(&run(args).await?);
    v["warning"] = json!(warning);
    Ok(v)
}

/// Is eep set up here (Settings › System)?
#[tauri::command]
pub async fn eep_doctor() -> Result<Value, String> {
    if !present() {
        return Ok(json!({ "available": false }));
    }
    Ok(doctor_of(&run(&["doctor"]).await?))
}

/// For the 6-hourly notification: `eep updates --count`, 0 without eep.
pub async fn update_count() -> usize {
    if !present() {
        return 0;
    }
    run(&["updates", "--count"])
        .await
        .ok()
        .and_then(|v| v.get("count").and_then(Value::as_u64))
        .unwrap_or(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_never_become_flags() {
        assert!(check_id("firefox").is_ok());
        assert!(check_id("telegram-desktop").is_ok());
        assert!(check_id("vscodium_1.2").is_ok());
        assert!(check_id("").is_err());
        assert!(check_id("--no-patches").is_err());
        assert!(check_id("-x").is_err());
        assert!(check_id("fire fox").is_err());
        assert!(check_id("a;rm").is_err());
        assert!(check_id("../x").is_err());
    }

    #[test]
    fn ok_false_is_a_failure_whatever_the_exit_status() {
        assert_eq!(
            reply("{\"ok\":false,\"error\":\"no app \\\"x\\\" in the index\"}").unwrap_err(),
            "no app \"x\" in the index"
        );
        assert_eq!(
            reply("{\"ok\":false}").unwrap_err(),
            "eep failed without saying why"
        );
        assert!(reply("{\"ok\":true,\"count\":2}").is_ok());
        // the object is the last line; anything before it is not the reply
        assert_eq!(
            reply("warming up\n{\"ok\":true,\"count\":3}\n\n").unwrap()["count"],
            3
        );
        // pretty-printed still reads
        assert!(reply("{\n  \"ok\": true\n}\n").is_ok());
        assert!(reply("").is_err());
        assert!(reply("Segmentation fault").is_err());
        assert!(reply("[1,2]").is_err());
        // what eep said on stderr when it printed nothing
        assert_eq!(
            with_stderr(reply(""), "thread 'main' panicked\n").unwrap_err(),
            "eep gave no answer Komble could read: thread 'main' panicked"
        );
        // …but never instead of its own error
        assert_eq!(
            with_stderr(reply("{\"ok\":false,\"error\":\"x\"}"), "noise").unwrap_err(),
            "x"
        );
    }

    #[tokio::test]
    async fn the_reply_decides_not_the_exit_status() {
        let dir = std::env::temp_dir().join(format!("komble-eep-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a stand-in eep, run through sh (never exec'd: no ETXTBSY race
        // with other tests forking while it is written)
        let script = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            p.to_string_lossy().into_owned()
        };
        let failed_ok = script("a", "echo '{\"ok\":false,\"error\":\"offline\"}'; exit 0");
        let ok_nonzero = script("b", "echo '{\"ok\":true,\"count\":1}'; exit 3");
        let silent = script("c", "echo boom >&2; exit 1");
        assert_eq!(run_bin("sh", &[&failed_ok]).await.unwrap_err(), "offline");
        assert_eq!(run_bin("sh", &[&ok_nonzero]).await.unwrap()["count"], 1);
        assert_eq!(
            run_bin("sh", &[&silent]).await.unwrap_err(),
            "eep gave no answer Komble could read: boom"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fetch_progress_streams_while_eep_runs() {
        let dir = std::env::temp_dir().join(format!("komble-eep-stream-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("install");
        std::fs::write(
            &p,
            "echo 'these 2 paths will be fetched (8.40 MiB download, 26.20 MiB unpacked):' >&2\n\
             echo \"copying path '/nix/store/a' from 'https://cache.nixos.org'...\" >&2\n\
             echo \"copying path '/nix/store/b' from 'file:///var/cache/eep/local'...\" >&2\n\
             echo '{\"ok\":true,\"id\":\"gimp\",\"download\":8808038}'\n",
        )
        .unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        let r = run_streamed("sh", &[&p.to_string_lossy()], move |f| {
            log.lock().unwrap().push((f.done, f.total))
        })
        .await
        .unwrap();
        assert_eq!(r["download"], 8808038);
        assert_eq!(*seen.lock().unwrap(), vec![(0, 2), (1, 2), (2, 2)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn available_apps_are_normalised() {
        let v = json!({
            "ok": true, "release": "nixos-26.11pre1125864.2786a2a72088",
            "apps": [
                { "id": "firefox", "name": "Firefox", "kind": "gui", "version": "160.0.1",
                  "description": "Web browser built from Firefox source tree",
                  "icon": "icons/firefox.png", "icon_file": null,
                  "icon_url": "https://eep.example/nixos-26.11pre1125864.2786a2a72088/icons/firefox.png",
                  "homepage": "http://www.mozilla.com/en-US/firefox/",
                  "download": 470707561u64, "disk": 1599532304u64, "unavailable": null,
                  "installed": "159.0", "update": true },
                // hard line breaks, no icon, no name, not installed
                { "id": "vscodium", "kind": "gui", "version": "1.106.3",
                  "description": "Open source source code editor developed by Microsoft for Windows,\nLinux and macOS\n",
                  "icon": null, "icon_url": null, "homepage": "javascript:alert(1)",
                  "download": 565890662u64, "disk": 1746089728u64,
                  "unavailable": "build failed on hydra", "installed": null, "update": false },
                // nothing to act on
                { "name": "no id" },
                { "id": "--flag" }
            ]
        });
        let apps = apps_of(&v);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].id, "firefox");
        assert_eq!(apps[0].installed.as_deref(), Some("159.0"));
        assert!(apps[0].update);
        assert_eq!(apps[0].unavailable, "");
        assert_eq!(apps[0].download, 470707561);
        assert_eq!(apps[1].name, "vscodium");
        assert_eq!(
            apps[1].description,
            "Open source source code editor developed by Microsoft for Windows, Linux and macOS"
        );
        assert_eq!(apps[1].icon, "");
        assert_eq!(apps[1].homepage, "");
        assert_eq!(apps[0].homepage, "http://www.mozilla.com/en-US/firefox/");
        assert_eq!(apps[1].installed, None);
        assert_eq!(apps[1].unavailable, "build failed on hydra");
        // no local copy: the https icon_url
        assert_eq!(
            apps[0].icon,
            "https://eep.example/nixos-26.11pre1125864.2786a2a72088/icons/firefox.png"
        );
        assert!(apps_of(&json!({ "ok": true })).is_empty());
    }

    #[test]
    fn icons_come_from_eeps_local_copy() {
        let dir = std::env::temp_dir().join(format!("komble-eep-icons-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("firefox.png");
        let svg = dir.join("gimp.SVG");
        std::fs::write(&png, b"\x89PNG").unwrap();
        std::fs::write(&svg, b"<svg/>").unwrap();
        let app = |file: &Path, url: &str| json!({ "id": "x", "icon_file": file.to_string_lossy(), "icon_url": url });
        assert_eq!(
            icon_of(&app(&png, "https://eep.example/x.png")),
            "data:image/png;base64,iVBORw=="
        );
        assert_eq!(
            icon_of(&app(&svg, "")),
            "data:image/svg+xml;base64,PHN2Zy8+"
        );
        // cached by mtime: a rewritten file is read again
        std::fs::write(&svg, b"<svg></svg>").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&svg)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(
            icon_of(&app(&svg, "")),
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="
        );
        // a missing file falls back to an https icon_url, never to http
        let gone = dir.join("gone.png");
        assert_eq!(icon_of(&app(&gone, "https://e/x.png")), "https://e/x.png");
        assert_eq!(
            icon_of(&app(&gone, "http://10.0.2.2:8000/r/icons/x.png")),
            ""
        );
        // relative paths and other types are not read
        assert_eq!(
            icon_of(&json!({ "id": "x", "icon_file": "firefox.png" })),
            ""
        );
        std::fs::write(dir.join("x.xpm"), b"/* XPM */").unwrap();
        assert_eq!(icon_of(&app(&dir.join("x.xpm"), "")), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base64_vectors() {
        for (plain, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), enc);
        }
    }

    #[test]
    fn change_replies_in_every_shape() {
        // one app: its fields at the top level, next to the totals
        let one = changed_of(&json!({
            "ok": true, "release": "r", "id": "firefox", "version": "160.0.1", "from": null,
            "out": "/nix/store/x-firefox-160.0.1", "card": "/home/me/Applications/Firefox.eep",
            "download_full": 121841800u64, "pieces_new": 38, "integrated": {},
            "download": 34150959u64, "patches": {}, "fell_back": false, "seconds": 14.9
        }));
        assert_eq!(one["download"], 34150959);
        assert_eq!(one["downloadFull"], 121841800);
        assert_eq!(one["seconds"], 14.9);
        assert_eq!(
            one["apps"],
            json!([{ "id": "firefox", "version": "160.0.1" }])
        );
        // several: under `apps`
        let many = changed_of(&json!({
            "ok": true, "release": "r", "download": 900, "download_full": 1000, "seconds": 3.0,
            "apps": [ { "id": "gimp", "version": "3.2.8" }, { "id": "vlc", "version": "3.0.24" } ]
        }));
        assert_eq!(many["apps"].as_array().unwrap().len(), 2);
        // update: under `updated`
        let up = changed_of(&json!({
            "ok": true, "download": 3215872, "download_full": 34150959,
            "updated": [ { "id": "firefox", "version": "160.0.1", "from": "159.0.2" } ]
        }));
        assert_eq!(up["apps"][0]["id"], "firefox");
        assert_eq!(up["download"], 3215872);
        // remove of one and of several
        let gone = changed_of(
            &json!({ "ok": true, "id": "gimp", "card": "/home/me/Applications/GIMP.eep" }),
        );
        assert_eq!(gone["apps"], json!([{ "id": "gimp", "version": "" }]));
        assert_eq!(gone["download"], 0);
        let gone2 =
            changed_of(&json!({ "ok": true, "apps": [ { "id": "gimp" }, { "id": "vlc" } ] }));
        assert_eq!(gone2["apps"].as_array().unwrap().len(), 2);
        // nothing pending for `eep update`
        assert_eq!(
            changed_of(&json!({ "ok": true, "updated": [] }))["apps"],
            json!([])
        );
    }

    #[test]
    fn installed_cards() {
        let v = json!({ "ok": true, "bad_cards": [], "apps": [
            { "id": "ripgrep", "name": "ripgrep", "kind": "cli", "version": "15.2.0",
              "release": "r", "out": "/nix/store/x-ripgrep-15.2.0", "present": true,
              "card": "/home/me/Applications/ripgrep.eep", "previous_version": null },
            { "id": "gimp", "kind": "gui", "version": "3.2.8" }
        ]});
        let l = installed_of(&v);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].kind, "cli");
        assert_eq!(l[1].name, "gimp");
    }

    #[test]
    fn updates_full_and_count_only() {
        let full = updates_of(&json!({
            "ok": true, "release": "r", "count": 1, "download": 3210000, "download_full": 34150959,
            "updates": [ { "id": "firefox", "name": "Firefox", "from": "159.0", "to": "160.0.1",
                           "rebuild_only": false, "download": 3210000, "download_full": 34150959,
                           "pieces": 38, "patched": 30 } ]
        }));
        assert_eq!(full["count"], 1);
        assert_eq!(full["downloadFull"], 34150959);
        assert_eq!(full["updates"][0]["from"], "159.0");
        assert_eq!(full["updates"][0]["rebuildOnly"], false);
        assert_eq!(full["updates"][0]["downloadFull"], 34150959);
        let count = updates_of(&json!({ "ok": true, "count": 4 }));
        assert_eq!(count["count"], 4);
        assert_eq!(count["updates"], json!([]));
    }

    #[test]
    fn doctor_names_what_stops_an_install() {
        let ready = doctor_of(&json!({
            "ok": true, "ready": true, "index": { "release": "r" },
            "nix": { "store": "daemon", "error": null }, "driver": { "ok": true }
        }));
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["problems"], json!([]));
        // eep's own verdict counts, with a hint when nothing below explains it
        let says_no = doctor_of(&json!({
            "ok": true, "ready": false, "index": { "release": "r" },
            "nix": { "store": "daemon", "error": null }, "driver": { "ok": true }
        }));
        assert_eq!(says_no["ready"], false);
        assert_eq!(says_no["problems"].as_array().unwrap().len(), 1);
        // an eep from before `ready`: the problems decide
        let not = doctor_of(&json!({
            "ok": true, "index": null, "nix": { "store": null, "error": "cannot connect to socket" },
            "driver": { "ok": false }
        }));
        assert_eq!(not["ready"], false);
        assert_eq!(not["problems"].as_array().unwrap().len(), 3);
        assert!(not["problems"][0]
            .as_str()
            .unwrap()
            .contains("cannot connect to socket"));
    }

    #[test]
    fn fetch_progress_from_nix_lines() {
        let mut f = Fetching::default();
        assert!(!f.feed("copying path '/nix/store/a' from 'https://cache.nixos.org'..."));
        assert!(f.feed("these 3 paths will be fetched (34.15 MiB download, 121.84 MiB unpacked):"));
        assert!(!f.feed("  /nix/store/aaa-glibc-2.42"));
        assert!(!f.feed("these 2 derivations will be built:"));
        assert!(f.feed("copying path '/nix/store/a' from 'file:///var/cache/eep/local'..."));
        assert!(f.feed("copying path '/nix/store/b' from 'https://cache.nixos.org'..."));
        assert_eq!(f, Fetching { total: 3, done: 2 });
        // a retry without patches fetches again: never past the total
        assert!(f.feed("this path will be fetched (0.01 MiB download, 0.03 MiB unpacked):"));
        for _ in 0..5 {
            f.feed("copying path '/nix/store/c' from 'https://cache.nixos.org'...");
        }
        assert_eq!(f, Fetching { total: 4, done: 4 });
    }
}
