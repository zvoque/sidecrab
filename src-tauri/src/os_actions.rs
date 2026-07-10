//! Tauri commands the webview invokes: window control, host activation, config.

use crate::config::{self, Config};
use tauri::{AppHandle, Manager, WebviewWindow};

// Window logical sizes per setting. Height keeps the sprite's 51:48 aspect so the
// crab is never distorted.
const SIZES: [(&str, f64, f64); 3] = [
    ("S", 102.0, 96.0),
    ("M", 153.0, 144.0),
    ("L", 204.0, 192.0),
];

pub fn logical_size(size: &str) -> (f64, f64) {
    SIZES
        .iter()
        .find(|(s, _, _)| *s == size)
        .map(|&(_, w, h)| (w, h))
        .unwrap_or((153.0, 144.0))
}

/// Absolute placement (wander walking / teleport home).
#[tauri::command]
pub fn set_window_pos(window: WebviewWindow, x: i32, y: i32) {
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Window + current-monitor rects in physical px, for wander pathing bounds.
#[tauri::command]
pub fn get_geometry(window: WebviewWindow) -> Option<serde_json::Value> {
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let mon = window.current_monitor().ok()??;
    let mp = mon.position();
    let ms = mon.size();
    Some(serde_json::json!({
        "winX": pos.x, "winY": pos.y,
        "winW": size.width, "winH": size.height,
        "monX": mp.x, "monY": mp.y,
        "monW": ms.width, "monH": ms.height,
    }))
}

/// Persist the current window position (called on drag end).
#[tauri::command]
pub fn persist_position(window: WebviewWindow) {
    if let Ok(pos) = window.outer_position() {
        let mut c = config::load();
        c.position = Some((pos.x, pos.y));
        let _ = config::save(&c);
    }
}

/// Bring the app hosting the active Claude Code session to the front.
/// `host` comes from TERM_PROGRAM recorded by the hook; "Claude" = desktop app.
#[tauri::command]
pub fn activate_host(host: String) {
    let app_name = match host.as_str() {
        "" | "Claude" => "Claude",
        "Apple_Terminal" => "Terminal",
        "iTerm.app" => "iTerm",
        "vscode" => "Visual Studio Code",
        "ghostty" => "Ghostty",
        other => other, // best-effort: try the raw TERM_PROGRAM value
    };
    let _ = std::process::Command::new("open")
        .args(["-a", app_name])
        .spawn();
}

#[tauri::command]
pub fn resize_window(window: WebviewWindow, size: String) {
    let (lw, lh) = logical_size(&size);
    // Anchor the bottom-right corner. Compute the new physical size instead of
    // querying outer_size() after set_size — the query returns the STALE size
    // (resize applies asynchronously), which made S/L drift.
    let before = (window.outer_position().ok(), window.outer_size().ok());
    let scale = window.scale_factor().unwrap_or(2.0);
    let _ = window.set_size(tauri::LogicalSize::new(lw, lh));
    let mut c = config::load();
    if let (Some(pos), Some(old)) = before {
        let (new_w, new_h) = ((lw * scale).round() as i32, (lh * scale).round() as i32);
        let (x, y) = (
            pos.x + old.width as i32 - new_w,
            pos.y + old.height as i32 - new_h,
        );
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        // Update the persisted HOME only if the crab was actually at home when
        // resized. Resizing a displaced (wandering/stranded) crab must not turn
        // its current spot into the new home — only dragging redefines home.
        let at_home = c
            .position
            .map(|(hx, hy)| (hx - pos.x).abs() <= 12 && (hy - pos.y).abs() <= 12)
            .unwrap_or(true);
        if at_home {
            c.position = Some((x, y));
        }
    }
    c.size = size;
    let _ = config::save(&c);
}

#[tauri::command]
pub fn get_config() -> Config {
    config::load()
}

#[tauri::command]
pub fn set_wander(enabled: bool) {
    let mut c = config::load();
    c.wander_enabled = enabled;
    let _ = config::save(&c);
}

/// Consent-gated hook management (consent recorded by the dialog flow).
#[tauri::command]
pub fn hooks_install(app: AppHandle) -> Result<(), String> {
    let bin = hook_bin_path(&app).ok_or("hook binary not found")?;
    crate::hook_installer::install_hooks(&crate::paths::claude_settings_path(), &bin)
        .map_err(|e| e.to_string())?;
    let mut c = config::load();
    c.hooks_consent = true;
    let _ = config::save(&c);
    Ok(())
}

#[tauri::command]
pub fn hooks_remove() -> Result<(), String> {
    crate::hook_installer::remove_hooks(&crate::paths::claude_settings_path())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hooks_status() -> bool {
    crate::hook_installer::hooks_installed(&crate::paths::claude_settings_path())
}

/// The bundled clawd-pet-hook sits next to the app executable.
fn hook_bin_path(app: &AppHandle) -> Option<String> {
    let exe = tauri::process::current_binary(&app.env()).ok()?;
    Some(exe.parent()?.join("clawd-pet-hook").to_string_lossy().into_owned())
}

