pub mod config;
pub mod hook_installer;
pub mod idle_monitor;
pub mod os_actions;
pub mod paths;
pub mod state_watcher;

use std::sync::Mutex;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::UpdaterExt;

/// Check GitHub releases; on confirmation download, install over the old app,
/// and relaunch. Silent about errors beyond a dialog — never blocks the pet.
fn check_for_updates(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(u) => u,
            Err(_) => return,
        };
        match updater.check().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                let handle = app.clone();
                app.dialog()
                    .message(format!(
                        "Version {version} is available (you have {}).\n\nDownload and install now?",
                        handle.package_info().version
                    ))
                    .title("Update available")
                    .buttons(MessageDialogButtons::OkCancelCustom("Update".into(), "Later".into()))
                    .show(move |yes| {
                        if !yes {
                            return;
                        }
                        tauri::async_runtime::spawn(async move {
                            if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
                                handle.restart();
                            } else {
                                handle
                                    .dialog()
                                    .message("Update failed to install. Grab it manually from GitHub releases.")
                                    .title("Update error")
                                    .show(|_| {});
                            }
                        });
                    });
            }
            Ok(None) => {
                app.dialog()
                    .message("You're on the latest version.")
                    .title("No updates")
                    .show(|_| {});
            }
            Err(e) => {
                app.dialog()
                    .message(format!("Update check failed: {e}"))
                    .title("Update error")
                    .show(|_| {});
            }
        }
    });
}

const CONSENT_TEXT: &str = "To react to Claude Code activity, Clawd Pet adds hooks to \
~/.claude/settings.json.\n\nYour file is backed up to settings.json.bak first, existing \
hooks are kept untouched, and you can remove ours anytime via right-click → \
\"Remove Claude Code hooks\".\n\nEnable activity detection?";

/// First-run disclaimer. No settings.json edit happens without an explicit yes;
/// declining leaves the crab inert (idle only) until enabled from the menu.
fn maybe_ask_consent(app: &AppHandle) {
    let cfg = config::load();
    if cfg.consent_asked
        || cfg.hooks_consent
        || hook_installer::hooks_installed(&paths::claude_settings_path())
    {
        return;
    }
    let handle = app.clone();
    app.dialog()
        .message(CONSENT_TEXT)
        .title("Clawd Pet")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Enable".into(),
            "Not now".into(),
        ))
        .show(move |accepted| {
            let mut c = config::load();
            c.consent_asked = true;
            let _ = config::save(&c);
            if accepted {
                let _ = os_actions::hooks_install(handle);
            }
        });
}

/// Opaque region of the sprite as fractions of the window (x0,y0,x1,y1), pushed by
/// the frontend. Used by the click-through poller — the crab is boxy, so a rect is
/// an accurate hit shape.
pub struct OpaqueRect(pub Mutex<(f64, f64, f64, f64)>);
/// While the user drags the crab we must not flip ignore_cursor_events mid-drag.
pub struct DragLock(pub Mutex<bool>);

#[tauri::command]
fn set_opaque_rect(state: tauri::State<OpaqueRect>, x0: f64, y0: f64, x1: f64, y1: f64) {
    *state.0.lock().unwrap() = (x0, y0, x1, y1);
}

#[tauri::command]
fn set_drag_lock(state: tauri::State<DragLock>, locked: bool) {
    *state.0.lock().unwrap() = locked;
}

/// The one settings menu, built fresh so checkmarks reflect current config.
/// Used as the right-click popup AND (wrapped) as the macOS app menu.
fn build_settings_menu(app: &AppHandle) -> Option<tauri::menu::Submenu<tauri::Wry>> {
    let win = app.get_webview_window("main")?;
    let cfg = config::load();
    let hooks_on = hook_installer::hooks_installed(&paths::claude_settings_path());
    let corner = os_actions::current_corner(&win);

    let size = SubmenuBuilder::new(app, "Size")
        .items(&[
            &CheckMenuItemBuilder::with_id("size-S", "Small").checked(cfg.size == "S").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("size-M", "Medium").checked(cfg.size == "M").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("size-L", "Large").checked(cfg.size == "L").build(app).ok()?,
        ])
        .build()
        .ok()?;

    let position = SubmenuBuilder::new(app, "Position")
        .items(&[
            &CheckMenuItemBuilder::with_id("pos-tl", "Top Left").checked(corner == Some("tl")).build(app).ok()?,
            &CheckMenuItemBuilder::with_id("pos-tr", "Top Right").checked(corner == Some("tr")).build(app).ok()?,
            &CheckMenuItemBuilder::with_id("pos-bl", "Bottom Left").checked(corner == Some("bl")).build(app).ok()?,
            &CheckMenuItemBuilder::with_id("pos-br", "Bottom Right").checked(corner == Some("br")).build(app).ok()?,
        ])
        .separator()
        .items(&[&MenuItemBuilder::with_id("pos-reset", "Reset Position").build(app).ok()?])
        .build()
        .ok()?;

    let hat = SubmenuBuilder::new(app, "Hat")
        .items(&[
            &CheckMenuItemBuilder::with_id("hat-none", "None").checked(cfg.hat == "none").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("hat-top", "Top Hat").checked(cfg.hat == "top").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("hat-chef", "Chef's Hat").checked(cfg.hat == "chef").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("hat-fedora", "Fedora").checked(cfg.hat == "fedora").build(app).ok()?,
            &CheckMenuItemBuilder::with_id("hat-heli", "Helicopter Hat").checked(cfg.hat == "heli").build(app).ok()?,
        ])
        .build()
        .ok()?;

    let wander = CheckMenuItemBuilder::with_id("wander", "Wander when idle")
        .checked(cfg.wander_enabled)
        .build(app)
        .ok()?;

    let hooks = if hooks_on {
        MenuItemBuilder::with_id("hooks-remove", "Remove Claude Code hooks").build(app).ok()?
    } else {
        MenuItemBuilder::with_id("hooks-install", "Enable activity detection…").build(app).ok()?
    };

    SubmenuBuilder::new(app, "Clawd Pet")
        .item(&size)
        .item(&position)
        .item(&hat)
        .item(&wander)
        .separator()
        .item(&hooks)
        .separator()
        .items(&[
            &MenuItemBuilder::with_id("update-check", "Check for Updates…").build(app).ok()?,
            &MenuItemBuilder::with_id("quit", "Quit Clawd Pet")
                .accelerator("CmdOrCtrl+Q")
                .build(app)
                .ok()?,
        ])
        .build()
        .ok()
}

/// macOS menu bar: the settings live under the app-name menu (no tray icon).
pub(crate) fn refresh_app_menu(app: &AppHandle) {
    if let Some(settings) = build_settings_menu(app) {
        if let Ok(menu) = MenuBuilder::new(app).item(&settings).build() {
            let _ = app.set_menu(menu);
        }
    }
}

#[tauri::command]
fn show_menu(window: WebviewWindow) {
    let Some(menu) = build_settings_menu(window.app_handle()) else { return };
    // Anchor high enough that the menu never opens past the screen bottom
    // (macOS renders a clipped, scroll-to-reveal menu otherwise).
    const MENU_H: f64 = 280.0; // generous logical estimate
    let y = match (window.current_monitor(), window.outer_position(), window.scale_factor()) {
        (Ok(Some(mon)), Ok(pos), Ok(scale)) => {
            let below = (mon.position().y + mon.size().height as i32 - pos.y) as f64 / scale;
            (below - MENU_H).min(0.0)
        }
        _ => -150.0,
    };
    let _ = window.popup_menu_at(&menu, tauri::Position::Logical(tauri::LogicalPosition::new(0.0, y)));
}

fn on_menu(app: &AppHandle, id: &str) {
    let win = app.get_webview_window("main");
    match id {
        "size-S" | "size-M" | "size-L" => {
            if let Some(w) = win {
                os_actions::resize_window(w, id.trim_start_matches("size-").to_string());
            }
        }
        // Corner placement becomes the new home; reset = default bottom-right.
        "pos-tl" | "pos-tr" | "pos-bl" | "pos-br" => {
            if let Some(w) = win {
                os_actions::place_corner(&w, id.trim_start_matches("pos-"), true);
            }
        }
        "pos-reset" => {
            if let Some(w) = win {
                os_actions::place_corner(&w, "br", true);
            }
        }
        "wander" => {
            let enabled = !config::load().wander_enabled;
            os_actions::set_wander(enabled);
            let _ = app.emit("wander-changed", enabled);
        }
        id if id.starts_with("hat-") => {
            let hat = id.trim_start_matches("hat-").to_string();
            let mut c = config::load();
            c.hat = hat.clone();
            let _ = config::save(&c);
            let _ = app.emit("hat-changed", hat);
        }
        // Menu action = explicit user intent = consent.
        "hooks-install" => {
            let _ = os_actions::hooks_install(app.clone());
        }
        "hooks-remove" => {
            let _ = os_actions::hooks_remove();
        }
        "update-check" => check_for_updates(app.clone()),
        "quit" => app.exit(0),
        _ => {}
    }
    refresh_app_menu(app); // keep menu-bar checkmarks in sync with the change
}

/// Poll the global cursor; make empty window pixels click-through. When the cursor
/// is outside the sprite's opaque rect the window ignores mouse events, so clicks
/// land on whatever is underneath.
fn spawn_click_through_poller(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring = false;
        let mut hovering = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(120));
            let Some(win) = app.get_webview_window("main") else { continue };
            if *app.state::<DragLock>().0.lock().unwrap() {
                continue;
            }
            let (Ok(cursor), Ok(pos), Ok(size)) =
                (app.cursor_position(), win.outer_position(), win.outer_size())
            else {
                continue;
            };
            let (lx, ly) = (cursor.x - pos.x as f64, cursor.y - pos.y as f64);
            let inside_window =
                lx >= 0.0 && ly >= 0.0 && lx < size.width as f64 && ly < size.height as f64;
            let over_crab = if inside_window {
                let (fx, fy) = (lx / size.width as f64, ly / size.height as f64);
                let (x0, y0, x1, y1) = *app.state::<OpaqueRect>().0.lock().unwrap();
                fx >= x0 && fx <= x1 && fy >= y0 && fy <= y1
            } else {
                false
            };
            // The same over_crab signal drives the hover reaction in the frontend.
            if over_crab != hovering {
                hovering = over_crab;
                let _ = app.emit("crab-hover", hovering);
            }
            // Ignore events only while the cursor is over empty pixels of our window;
            // outside the window the flag is irrelevant, so reset it for safety.
            let want_ignore = inside_window && !over_crab;
            if want_ignore != ignoring {
                ignoring = want_ignore;
                let _ = win.set_ignore_cursor_events(ignoring);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Second launch = no twin crabs; the existing instance just stays.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(OpaqueRect(Mutex::new((0.0, 0.0, 1.0, 1.0))))
        .manage(DragLock(Mutex::new(false)))
        .invoke_handler(tauri::generate_handler![
            set_opaque_rect,
            set_drag_lock,
            show_menu,
            os_actions::set_window_pos,
            os_actions::get_geometry,
            os_actions::cursor_pos,
            os_actions::persist_position,
            os_actions::activate_host,
            os_actions::resize_window,
            os_actions::get_config,
            os_actions::set_wander,
            os_actions::hooks_install,
            os_actions::hooks_remove,
            os_actions::hooks_status,
            idle_monitor::user_is_idle,
        ])
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            // Float above other apps on every Space.
            let _ = win.set_visible_on_all_workspaces(true);

            let cfg = config::load();
            let (lw, lh) = os_actions::logical_size(&cfg.size);
            let _ = win.set_size(tauri::LogicalSize::new(lw, lh));
            match cfg.position {
                Some((x, y)) => {
                    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                }
                None => {
                    // Default resting spot: bottom-right corner (not persisted — a
                    // saved home only comes from dragging or the Position menu).
                    os_actions::place_corner(&win, "br", false);
                }
            }

            app.on_menu_event(|app, event| on_menu(app, event.id().as_ref()));
            refresh_app_menu(app.handle()); // settings under the app-name menu too
            state_watcher::spawn(app.handle().clone());
            spawn_click_through_poller(app.handle().clone());
            idle_monitor::spawn(app.handle().clone());
            maybe_ask_consent(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
