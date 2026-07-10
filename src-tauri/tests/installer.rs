// Tests for hook_installer: merging our hook entries into ~/.claude/settings.json
// must be consent-gated upstream, backed up, additive, idempotent, and removable
// without touching anyone else's hooks (e.g. an existing CSB install).
use clawd_pet_lib::hook_installer::{hooks_installed, install_hooks, remove_hooks};
use serde_json::{json, Value};
use std::path::PathBuf;

const EVENTS: [&str; 8] = [
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "PermissionRequest",
    "Stop",
    "SessionStart",
    "SessionEnd",
];

fn tmp_settings(name: &str, content: &Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("clawd-installer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, content.to_string()).unwrap();
    path
}

fn read(path: &PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn count_ours(v: &Value, event: &str) -> usize {
    v["hooks"][event]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e.to_string().contains("clawd-pet-hook"))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn installs_into_empty_settings_with_backup() {
    let path = tmp_settings("empty", &json!({}));
    install_hooks(&path, "/apps/Clawd Pet.app/MacOS/clawd-pet-hook").unwrap();

    let v = read(&path);
    for ev in EVENTS {
        assert_eq!(count_ours(&v, ev), 1, "missing hook for {ev}");
    }
    assert!(path.with_extension("json.bak").exists(), "backup missing");
    assert!(hooks_installed(&path));
}

#[test]
fn install_is_idempotent() {
    let path = tmp_settings("idem", &json!({}));
    let bin = "/x/clawd-pet-hook";
    install_hooks(&path, bin).unwrap();
    install_hooks(&path, bin).unwrap();
    let v = read(&path);
    for ev in EVENTS {
        assert_eq!(count_ours(&v, ev), 1, "duplicated hook for {ev}");
    }
}

#[test]
fn preserves_existing_hooks_and_removes_only_ours() {
    let csb = json!({
        "permissions": { "allow": ["Bash(ls:*)"] },
        "hooks": {
            "PreToolUse": [
                { "matcher": "*", "hooks": [ { "type": "command", "command": "node /Users/z/.claude/statusbar/update.js pre" } ] }
            ],
            "Stop": [
                { "hooks": [ { "type": "command", "command": "node /Users/z/.claude/statusbar/update.js stop" } ] }
            ]
        }
    });
    let path = tmp_settings("csb", &csb);
    install_hooks(&path, "/x/clawd-pet-hook").unwrap();

    let v = read(&path);
    // CSB entries intact, ours added alongside.
    assert!(v["hooks"]["PreToolUse"].to_string().contains("statusbar/update.js"));
    assert_eq!(count_ours(&v, "PreToolUse"), 1);
    assert_eq!(v["permissions"]["allow"][0], "Bash(ls:*)");

    remove_hooks(&path).unwrap();
    let v = read(&path);
    assert!(!hooks_installed(&path));
    // CSB survives removal; our emptied event keys are pruned.
    assert!(v["hooks"]["PreToolUse"].to_string().contains("statusbar/update.js"));
    assert!(v["hooks"]["UserPromptSubmit"].is_null());
    assert_eq!(v["permissions"]["allow"][0], "Bash(ls:*)");
}

#[test]
fn backup_is_not_overwritten_by_reinstall() {
    let original = json!({"marker": "original"});
    let path = tmp_settings("bak", &original);
    install_hooks(&path, "/x/clawd-pet-hook").unwrap();
    remove_hooks(&path).unwrap();
    install_hooks(&path, "/x/clawd-pet-hook").unwrap();
    let bak: Value =
        serde_json::from_str(&std::fs::read_to_string(path.with_extension("json.bak")).unwrap())
            .unwrap();
    assert_eq!(bak["marker"], "original", "backup must keep the pristine original");
}
