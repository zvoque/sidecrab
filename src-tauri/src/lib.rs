pub mod hook_installer;
pub mod paths;
pub mod state_watcher;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            // Float above other apps and follow the user across every Space, so the
            // pet is always visible regardless of which desktop is active.
            let _ = win.set_visible_on_all_workspaces(true);
            // Default resting spot: bottom-right corner (saved position replaces
            // this once config persistence lands).
            if let (Ok(Some(mon)), Ok(size)) = (win.current_monitor(), win.outer_size()) {
                let m = mon.size();
                let margin = (24.0 * mon.scale_factor()) as i32;
                let x = m.width as i32 - size.width as i32 - margin;
                let y = m.height as i32 - size.height as i32 - margin;
                let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
            }
            state_watcher::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
