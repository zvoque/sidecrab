//! Detects user inactivity via macOS HIDIdleTime and emits `user-idle` /
//! `user-active` transition events. The wander behavior (frontend) consumes them.

use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_secs(2);
const DEFAULT_THRESHOLD_SECS: f64 = 60.0;

fn hid_idle_secs() -> Option<f64> {
    let out = std::process::Command::new("ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4", "-r", "-k", "HIDIdleTime"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.contains("HIDIdleTime"))?;
    let ns: f64 = line.split('=').nth(1)?.trim().parse().ok()?;
    Some(ns / 1e9)
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        // Override for testing: CLAWD_PET_IDLE_SECS=10
        let threshold: f64 = std::env::var("CLAWD_PET_IDLE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_THRESHOLD_SECS);
        let mut was_idle = false;
        loop {
            std::thread::sleep(POLL);
            let Some(idle) = hid_idle_secs() else { continue };
            let is_idle = idle >= threshold;
            if is_idle != was_idle {
                was_idle = is_idle;
                let _ = app.emit(if is_idle { "user-idle" } else { "user-active" }, is_idle);
            }
        }
    });
}
