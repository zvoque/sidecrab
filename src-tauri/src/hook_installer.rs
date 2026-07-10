//! Merges our Claude Code hook entries into ~/.claude/settings.json.
//! Contract (spec §4): consent-gated by the caller, pristine backup written once,
//! additive + idempotent (keyed on the hook binary marker), removable without
//! touching any other tool's hooks.

use serde_json::{json, Value};
use std::io;
use std::path::Path;

/// Stable marker identifying our entries regardless of install location.
const MARKER: &str = "clawd-pet-hook";

/// (settings event key, hook binary argument, needs "*" matcher)
const EVENTS: [(&str, &str, bool); 8] = [
    ("UserPromptSubmit", "prompt", false),
    ("PreToolUse", "pre", true),
    ("PostToolUse", "post", true),
    ("Notification", "notify", false),
    ("PermissionRequest", "permreq", true),
    ("Stop", "stop", false),
    ("SessionStart", "start", false),
    ("SessionEnd", "end", false),
];

fn read_settings(path: &Path) -> io::Result<Value> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_else(|_| json!({}))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e),
    }
}

fn write_atomic(path: &Path, v: &Value) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
    std::fs::rename(&tmp, path)
}

fn entry_is_ours(entry: &Value) -> bool {
    entry.to_string().contains(MARKER)
}

/// Add our hook entries. Backs up the pristine file once (never overwrites an
/// existing backup, so the original survives reinstall cycles).
pub fn install_hooks(settings_path: &Path, hook_bin: &str) -> io::Result<()> {
    let mut v = read_settings(settings_path)?;

    let bak = settings_path.with_extension("json.bak");
    if settings_path.exists() && !bak.exists() {
        std::fs::copy(settings_path, &bak)?;
    }

    if !v.is_object() {
        v = json!({});
    }
    let hooks = v
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }

    for (event, arg, matcher) in EVENTS {
        let arr = hooks
            .as_object_mut()
            .unwrap()
            .entry(event)
            .or_insert_with(|| json!([]));
        if !arr.is_array() {
            *arr = json!([]);
        }
        let arr = arr.as_array_mut().unwrap();
        if arr.iter().any(entry_is_ours) {
            continue; // idempotent
        }
        // Quote the binary path — the app bundle lives under "Application Support"/"…app".
        let cmd = format!("\"{}\" {}", hook_bin, arg);
        let mut entry = json!({ "hooks": [ { "type": "command", "command": cmd } ] });
        if matcher {
            entry["matcher"] = json!("*");
        }
        arr.push(entry);
    }

    write_atomic(settings_path, &v)
}

/// Strip only our entries; prune event keys we emptied. Everything else is untouched.
pub fn remove_hooks(settings_path: &Path) -> io::Result<()> {
    let mut v = read_settings(settings_path)?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let events: Vec<String> = hooks.keys().cloned().collect();
        for ev in events {
            if let Some(arr) = hooks.get_mut(&ev).and_then(|a| a.as_array_mut()) {
                arr.retain(|e| !entry_is_ours(e));
                if arr.is_empty() {
                    hooks.remove(&ev);
                }
            }
        }
    }
    write_atomic(settings_path, &v)
}

pub fn hooks_installed(settings_path: &Path) -> bool {
    read_settings(settings_path)
        .map(|v| v["hooks"].to_string().contains(MARKER))
        .unwrap_or(false)
}
