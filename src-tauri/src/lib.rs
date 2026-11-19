mod appimage;
mod catalog;
mod de;
mod first_party;
mod pacman;
mod plugins;
mod registry;
mod system;
mod util;

use tauri::{
    AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_store::StoreExt;

/// A file the user opened Komble WITH (double-click / "Open with" — the .desktop
/// entry passes it as an argv). Stashed because the page may not be listening
/// yet — at process start, and again whenever the window is built on demand
/// (a second launch while a transaction keeps a windowless Komble alive); the
/// frontend collects it via `take_pending_open` once mounted, and an open page
/// is told to come and get it (the `open-file` event carries no payload).
static PENDING_OPEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// A route the app was launched INTO (`komble --updates`, used by the DE bar's
/// update indicator). Same stash-then-collect pattern as PENDING_OPEN.
static PENDING_ROUTE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn stash(slot: &std::sync::Mutex<Option<String>>, v: String) {
    if let Ok(mut g) = slot.lock() {
        *g = Some(v);
    }
}

fn route_arg(arg: &str) -> Option<String> {
    match arg {
        "--updates" => Some("updates".into()),
        "--settings" => Some("settings".into()),
        // `komble --addons` from the shell (Shell.openStore("addons"), the
        // Welcome screen) and ewe-settings; the catalog is the top of Plugins
        // (`--addons` stays: installed binaries call it — Rule 4)
        "--addons" | "--plugins" => Some("plugins".into()),
        // `komble --search=pdf` — the desktop's gnome-software stand-in sends
        // GTK's "Find New Applications" here with the file/link type as words
        _ => arg
            .strip_prefix("--search=")
            .map(|q| format!("search:{}", q.trim()))
            .or_else(|| arg.strip_prefix("--options=").and_then(options_route)),
    }
}

/// `komble --options=ewe.dock` — Plugins with that plugin's sheet
/// open (ewe-settings' "Dock options", the shell). The id is checked here:
/// it travels into the webview as a route.
fn options_route(id: &str) -> Option<String> {
    let id = id.trim();
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    ok.then(|| format!("options:{id}"))
}

/// `--search=q` or `--search q`, `--options=id` or `--options id` from an
/// argv (the first route wins).
fn route_from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let v: Vec<String> = args.into_iter().collect();
    let mut i = 0;
    while i < v.len() {
        if v[i] == "--search" && i + 1 < v.len() {
            return Some(format!("search:{}", v[i + 1].trim()));
        }
        if v[i] == "--options" && i + 1 < v.len() {
            if let Some(r) = options_route(&v[i + 1]) {
                return Some(r);
            }
        }
        if let Some(r) = route_arg(&v[i]) {
            return Some(r);
        }
        i += 1;
    }
    None
}

#[tauri::command]
fn take_pending_route() -> Option<String> {
    PENDING_ROUTE.lock().ok().and_then(|mut g| g.take())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Some launchers hand over `file:///path/with%20spaces` rather than a path.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An argv entry we know how to install: an .AppImage or a pacman package file.
fn openable_path(arg: &str) -> Option<String> {
    let p = match arg.strip_prefix("file://") {
        Some(rest) => percent_decode(rest),
        None => arg.to_string(),
    };
    let low = p.to_lowercase();
    if low.ends_with(".appimage") || low.contains(".pkg.tar") {
        Some(p)
    } else {
        None
    }
}

#[tauri::command]
fn take_pending_open() -> Option<String> {
    PENDING_OPEN.lock().ok().and_then(|mut g| g.take())
}

const MAIN: &str = "main";

/// Show the window, building it when there is none. It is not declared in
/// tauri.conf.json any more: a window that exists only while it is on screen
/// means a WebKit page (and its ~130 MB of WebKitWebProcess) exists only
/// then. Closing it destroys it; lib.rs decides whether the process stays.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    // the same window tauri.conf.json declared until 0.22.1
    let built = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("Komble")
        .inner_size(1180.0, 760.0)
        .min_inner_size(420.0, 480.0)
        .center()
        .decorations(false)
        .build();
    if let Err(e) = built {
        eprintln!("komble: could not open the window: {e}");
    }
}

/// Close = destroy, and the WebKit web process with it. WebKit keeps a
/// page-less web process in its process cache for up to 30 minutes (~90 MB),
/// and a reopened window gets a fresh one anyway — so while a transaction
/// keeps a windowless Komble alive, the process it holds is the Rust side
/// alone. `terminate_web_process` runs synchronously on the main thread from
/// inside the close request, before the window is torn down; afterwards the
/// heap glibc kept from the page's lifetime is handed back to the kernel.
fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    match event {
        WindowEvent::CloseRequested { .. } => {
            if let Some(wv) = window.webviews().into_iter().next() {
                let _ = wv.with_webview(|pw| {
                    use webkit2gtk::WebViewExt;
                    pw.inner().terminate_web_process();
                });
            }
        }
        WindowEvent::Destroyed => {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(3));
                extern "C" {
                    fn malloc_trim(pad: usize) -> i32;
                }
                // glibc: give freed arenas back; Arch only, so glibc always
                unsafe { malloc_trim(0) };
            });
        }
        _ => {}
    }
}

fn setting_bool(app: &AppHandle, key: &str, default: bool) -> bool {
    app.store("settings.json")
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn setting_string(app: &AppHandle, key: &str) -> Option<String> {
    app.store("settings.json")
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.trim().is_empty())
}

/// Self-update via tauri-plugin-updater. Only functional when built with
/// `--features self-update` and a `plugins.updater` config (see README).
#[tauri::command]
async fn check_self_update(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(feature = "self-update")]
    {
        use tauri_plugin_updater::UpdaterExt;
        let updater = app.updater().map_err(|e| e.to_string())?;
        match updater.check().await {
            Ok(Some(update)) => Ok(Some(update.version.clone())),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    #[cfg(not(feature = "self-update"))]
    {
        let _ = app;
        Err("Self-updater is not enabled in this build (see README: self-update).".into())
    }
}

async fn background_update_loop(app: AppHandle) {
    // RFC-002: publish the installed-apps manifest into ewe.conf at startup
    // too — a machine that never installs anything still describes itself.
    registry::mirror_manifest(&app);
    // Installs from before extracted run dirs existed get one now, in the
    // background; a no-op once every entry is up to date.
    appimage::ensure_run_dirs(app.clone());
    // First check a few minutes after launch, then every 6 hours.
    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
    let mut last: i64 = -1;
    loop {
        if setting_bool(&app, "notifyUpdates", true) {
            let mut count = 0usize;
            if let Ok(list) = pacman::upgradable_inner().await {
                count += list.len();
            }
            let token = setting_string(&app, "githubToken");
            if let Ok(res) = appimage::check_updates_inner(&app, token).await {
                count += res.updates.len();
            }
            if count > 0 && count as i64 != last {
                use tauri_plugin_notification::NotificationExt;
                let _ = app
                    .notification()
                    .builder()
                    .title("Komble — updates available")
                    .body(format!("{count} update(s) ready to install."))
                    .show();
                let _ = app.emit("updates-count", count);
            }
            last = count as i64;
        }
        tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
    }
}

pub fn run() {
    if let Some(p) = std::env::args().skip(1).find_map(|a| openable_path(&a)) {
        stash(&PENDING_OPEN, p);
    }
    if let Some(r) = route_from_args(std::env::args().skip(1)) {
        stash(&PENDING_ROUTE, r);
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A second launch while Komble is alive (a transaction in flight,
            // or its window simply open): a file to open, a route to go to.
            // Stashed first, then the page is told — it may not exist yet
            // (show_main builds it and it mounts a moment later), in which
            // case it collects the stash on mount instead.
            if let Some(p) = args.iter().skip(1).find_map(|a| openable_path(a)) {
                stash(&PENDING_OPEN, p);
                let _ = app.emit("open-file", ());
            }
            // `komble --updates` from the bar indicator, or `--search=…` from
            // the desktop's store stand-in, while already running
            if let Some(r) = route_from_args(args.iter().skip(1).cloned()) {
                stash(&PENDING_ROUTE, r);
                let _ = app.emit("navigate", ());
            }
            show_main(app);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    #[cfg(feature = "self-update")]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    let app = builder
        .setup(|app| {
            // No tray icon — the DE bar's always-on indicator IS the tray
            // (font-icon states + update count, live even before first launch).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(background_update_loop(handle));
            show_main(app.handle());
            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            catalog::fetch_catalog,
            catalog::resolve_release,
            catalog::resolve_am_app,
            appimage::install_appimage,
            appimage::remove_appimage,
            appimage::list_appimages,
            appimage::check_appimage_updates,
            appimage::update_appimage,
            appimage::appimage_file_info,
            appimage::install_local_appimage,
            // repositories
            pacman::browse_packages,
            pacman::package_repos,
            pacman::package_info,
            pacman::install_package,
            pacman::install_packages,
            pacman::remove_package,
            pacman::list_tracked_packages,
            pacman::installed_package_names,
            pacman::package_file_info,
            pacman::install_package_file,
            // updates — note there is no per-REPO-package upgrade command on
            // purpose; partial upgrades are unsupported on Arch (see pacman.rs).
            // aur_upgrade rebuilds foreign packages, which -Syu never touches.
            pacman::list_upgradable,
            pacman::system_upgrade,
            pacman::system_upgrade_accept_removals,
            pacman::restart_state,
            pacman::restart_action,
            pacman::aur_upgrade,
            pacman::refresh_lists,
            // first-party ewe apps + the desktop itself
            first_party::first_party_status,
            first_party::first_party_installed,
            de::app_manifest,
            de::manifest_dump,
            first_party::install_first_party,
            first_party::ewe_status,
            first_party::ewe_update,
            first_party::ewe_update_terminal,
            // AUR
            pacman::aur_search,
            pacman::aur_pkgbuild,
            pacman::aur_srcinfo,
            pacman::aur_install,
            system::system_check,
            de::de_prefs,
            de::theme_tokens,
            de::poke_shell_updates,
            // shell plugins (ewe-plugin)
            plugins::plugin_list,
            plugins::plugin_install,
            plugins::plugin_add,
            plugins::plugin_set_enabled,
            plugins::plugin_update,
            plugins::plugin_remove,
            plugins::plugin_restore,
            plugins::plugin_create,
            plugins::plugin_set,
            plugins::plugin_place,
            plugins::plugin_bar,
            plugins::plugin_arrange,
            system::install_fuse2,
            system::install_pacman_contrib,
            take_pending_open,
            take_pending_route,
            check_self_update
        ])
        .build(tauri::generate_context!())
        .expect("error while building Komble");

    app.run(|app, event| {
        // Closing the window ends Komble: nothing of it needs to stay
        // resident (the bar's update indicator is the shell's own, fed by
        // updates-check.sh), and until 0.22.1 "minimize to tray" kept a
        // windowless process with a live WebKit page — ~250 MB — around
        // after `komble --updates`. The one exception is a transaction in
        // flight (util::Working — a pacman run under pkexec, a makepkg
        // build, an AppImage download): the window goes, the process stays
        // until the work is done, then leaves unless a window came back.
        if let RunEvent::ExitRequested { api, code: None, .. } = &event {
            if util::working() {
                api.prevent_exit();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    while util::working() {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                    if app.get_webview_window(MAIN).is_none() {
                        app.exit(0);
                    }
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(args: &[&str]) -> Option<String> {
        route_from_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn deep_links_route_to_their_pages() {
        assert_eq!(route(&["--updates"]).as_deref(), Some("updates"));
        assert_eq!(route(&["--settings"]).as_deref(), Some("settings"));
        assert_eq!(route(&["--addons"]).as_deref(), Some("plugins"));
        assert_eq!(route(&["--plugins"]).as_deref(), Some("plugins"));
        assert_eq!(
            route(&["--options=ewe.dock"]).as_deref(),
            Some("options:ewe.dock")
        );
        assert_eq!(
            route(&["--options", "acme.clock"]).as_deref(),
            Some("options:acme.clock")
        );
        assert_eq!(route(&["--options=Bad Id;rm"]), None);
    }

    #[test]
    fn search_takes_both_spellings() {
        assert_eq!(
            route(&["--search=pdf viewer"]).as_deref(),
            Some("search:pdf viewer")
        );
        assert_eq!(route(&["--search", " pdf "]).as_deref(), Some("search:pdf"));
    }

    #[test]
    fn unknown_args_route_nowhere() {
        assert_eq!(route(&[]), None);
        assert_eq!(route(&["/home/me/x.AppImage", "--verbose"]), None);
    }
}
