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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
