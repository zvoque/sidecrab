//! Watches state.json (written by clawd-pet-hook) and forwards each change to the
//! webview as a `claude-state` event. The pet is a pure consumer of that file.

use notify::{RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const DEBOUNCE: Duration = Duration::from_millis(120);

fn current_state(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({ "state": "idle" }))
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let dir = crate::paths::home();
        let _ = std::fs::create_dir_all(&dir);
        let state_path = crate::paths::state_path();

        // Initial emit so the crab reflects reality on launch.
        let _ = app.emit("claude-state", current_state(&state_path));

        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(_) => return,
        };
        // Watch the dir, not the file: atomic tmp+rename replaces the inode.
        if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
            return;
        }

        while let Ok(ev) = rx.recv() {
            let touches_state = matches!(&ev, Ok(e) if e
                .paths
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == "state.json")));
            if !touches_state {
                continue;
            }
            // Debounce: coalesce the write burst, then emit once.
            while rx.recv_timeout(DEBOUNCE).is_ok() {}
            let _ = app.emit("claude-state", current_state(&state_path));
        }
    });
}
