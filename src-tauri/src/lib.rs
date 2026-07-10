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

#[tauri::command]
fn show_menu(window: WebviewWindow) {
    let app = window.app_handle();
    let cfg = config::load();
    let hooks_on = hook_installer::hooks_installed(&paths::claude_settings_path());

    let size = SubmenuBuilder::new(app, "Size")
        .items(&[
            &CheckMenuItemBuilder::with_id("size-S", "Small").checked(cfg.size == "S").build(app).unwrap(),
            &CheckMenuItemBuilder::with_id("size-M", "Medium").checked(cfg.size == "M").build(app).unwrap(),
            &CheckMenuItemBuilder::with_id("size-L", "Large").checked(cfg.size == "L").build(app).unwrap(),
        ])
        .build()
        .unwrap();

    let wander = CheckMenuItemBuilder::with_id("wander", "Wander when idle")
        .checked(cfg.wander_enabled)
        .build(app)
        .unwrap();

    let hooks = if hooks_on {
        MenuItemBuilder::with_id("hooks-remove", "Remove Claude Code hooks").build(app).unwrap()
    } else {
        MenuItemBuilder::with_id("hooks-install", "Enable activity detection…").build(app).unwrap()
    };

    let menu = MenuBuilder::new(app)
        .item(&size)
        .item(&wander)
        .separator()
        .item(&hooks)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Clawd Pet").build(app).unwrap())
        .build()
        .unwrap();

    let _ = window.popup_menu(&menu);
}

fn on_menu(app: &AppHandle, id: &str) {
    let win = app.get_webview_window("main");
    match id {
        "size-S" | "size-M" | "size-L" => {
            if let Some(w) = win {
                os_actions::resize_window(w, id.trim_start_matches("size-").to_string());
            }
        }
        "wander" => {
            let enabled = !config::load().wander_enabled;
            os_actions::set_wander(enabled);
            let _ = app.emit("wander-changed", enabled);
        }
        // Menu action = explicit user intent = consent.
        "hooks-install" => {
            let _ = os_actions::hooks_install(app.clone());
        }
        "hooks-remove" => {
            let _ = os_actions::hooks_remove();
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Poll the global cursor; make empty window pixels click-through. When the cursor
/// is outside the sprite's opaque rect the window ignores mouse events, so clicks
/// land on whatever is underneath.
fn spawn_click_through_poller(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring = false;
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(OpaqueRect(Mutex::new((0.0, 0.0, 1.0, 1.0))))
        .manage(DragLock(Mutex::new(false)))
        .invoke_handler(tauri::generate_handler![
            set_opaque_rect,
            set_drag_lock,
            show_menu,
            os_actions::move_window_by,
            os_actions::set_window_pos,
            os_actions::get_geometry,
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
            let px = os_actions::size_px(&cfg.size);
            let _ = win.set_size(tauri::LogicalSize::new(px, px));
            match cfg.position {
                Some((x, y)) => {
                    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                }
                None => {
                    // Default resting spot: bottom-right corner.
                    if let (Ok(Some(mon)), Ok(size)) = (win.current_monitor(), win.outer_size()) {
                        let m = mon.size();
                        let margin = (24.0 * mon.scale_factor()) as i32;
                        let _ = win.set_position(tauri::PhysicalPosition::new(
                            m.width as i32 - size.width as i32 - margin,
                            m.height as i32 - size.height as i32 - margin,
                        ));
                    }
                }
            }

            app.on_menu_event(|app, event| on_menu(app, event.id().as_ref()));
            state_watcher::spawn(app.handle().clone());
            spawn_click_through_poller(app.handle().clone());
            idle_monitor::spawn(app.handle().clone());
            maybe_ask_consent(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
