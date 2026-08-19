mod appimage;
mod catalog;
mod de;
mod pacman;
mod registry;
mod system;
mod util;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_store::StoreExt;

/// A file the user opened Komble WITH (double-click / "Open with" — the .desktop
/// entry passes it as an argv). Stashed at startup because the webview isn't
/// listening yet; the frontend collects it via `take_pending_open` once mounted.
static PENDING_OPEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

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

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
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

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "Open Komble", true, None::<&str>)?;
    let updates_i = MenuItem::with_id(app, "updates", "Check for updates", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &updates_i, &quit_i])?;

    TrayIconBuilder::with_id("komble-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Komble")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "updates" => {
                show_main(app);
                let _ = app.emit("navigate", "updates");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    if let Some(p) = std::env::args().skip(1).find_map(|a| openable_path(&a)) {
        if let Ok(mut g) = PENDING_OPEN.lock() {
            *g = Some(p);
        }
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Second launch with a file (double-click while Komble runs in the
            // tray): forward it to the live instance.
            show_main(app);
            if let Some(p) = args.iter().skip(1).find_map(|a| openable_path(a)) {
                let _ = app.emit("open-file", p);
            }
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

    builder
        .setup(|app| {
            setup_tray(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(background_update_loop(handle));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main"
                    && setting_bool(window.app_handle(), "minimizeToTray", true)
                {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
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
            pacman::remove_package,
            pacman::list_tracked_packages,
            pacman::package_file_info,
            pacman::install_package_file,
            // updates — note there is no per-package upgrade command on purpose;
            // partial upgrades are unsupported on Arch (see pacman.rs)
            pacman::list_upgradable,
            pacman::system_upgrade,
            pacman::refresh_lists,
            // AUR
            pacman::aur_search,
            pacman::aur_pkgbuild,
            pacman::aur_install,
            system::system_check,
            de::de_prefs,
            de::qs_ipc,
            de::backup_packages,
            system::install_fuse2,
            system::install_pacman_contrib,
            take_pending_open,
            check_self_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running Komble");
}
