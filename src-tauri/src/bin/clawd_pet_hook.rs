// Standalone hook handler invoked by Claude Code hooks. Reads the hook JSON payload
// on stdin, maps the event to a pet state, and atomically writes state.json under
// CLAWD_PET_HOME (default: ~/Library/Application Support/clawd-pet). Also maintains
// sessions.d/ (one file per live session) and, on session start/end, clears a stale
// frozen state — but only if that state is owned by the same session id (a warmup
// burst from another session must never wipe a live turn).
//
// Usage: clawd-pet-hook <prompt|pre|post|notify|permreq|stop|start|end>

use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    if let Ok(h) = std::env::var("CLAWD_PET_HOME") {
        return PathBuf::from(h);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("clawd-pet")
}

fn tool_label(tool: &str) -> &'static str {
    match tool {
        "Bash" => "Running command",
        "Edit" | "MultiEdit" | "NotebookEdit" => "Editing",
        "Write" => "Writing",
        "Read" => "Reading",
        "Grep" | "Glob" => "Searching",
        "WebFetch" => "Browsing web",
        "WebSearch" => "Searching web",
        "Task" => "Delegating",
        "TodoWrite" => "Planning",
        _ => "Using tool",
    }
}

fn safe_id(v: &Value) -> String {
    v["session_id"]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "_.-".contains(*c))
        .take(64)
        .collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn read_state(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_atomic(path: &Path, v: &Value) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Reset a frozen mid-turn state, but only when `sid` owns it. Force-quit fires
/// SessionEnd with no Stop, which would otherwise freeze the crab mid-animation.
fn clear_stale_state(state_path: &Path, sid: &str) {
    let prev = read_state(state_path);
    if prev["sessionId"].as_str().unwrap_or("") != sid || sid.is_empty() {
        return;
    }
    match prev["state"].as_str().unwrap_or("") {
        "thinking" | "tool" | "permission" => {}
        _ => return,
    }
    let mut out = prev;
    out["state"] = json!("idle");
    out["label"] = json!("");
    out["startedAt"] = json!(0);
    out["ts"] = json!(now());
    write_atomic(state_path, &out);
}

fn main() {
    let event = std::env::args().nth(1).unwrap_or_default();
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let p: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));

    let dir = home();
    let state_path = dir.join("state.json");
    let sess_dir = dir.join("sessions.d");
    let sid = safe_id(&p);

    // Session lifecycle events only maintain the registry + stale-state guard.
    match event.as_str() {
        "start" => {
            let _ = std::fs::create_dir_all(&sess_dir);
            if !sid.is_empty() {
                let _ = std::fs::write(sess_dir.join(&sid), "");
            }
            clear_stale_state(&state_path, &sid);
            return;
        }
        "end" => {
            if !sid.is_empty() {
                let _ = std::fs::remove_file(sess_dir.join(&sid));
            }
            clear_stale_state(&state_path, &sid);
            return;
        }
        _ => {}
    }

    // Register the session on any activity too, so sessions predating hook install
    // are tracked once they do anything.
    if !sid.is_empty() {
        let _ = std::fs::create_dir_all(&sess_dir);
        let _ = std::fs::write(sess_dir.join(&sid), "");
    }

    let prev = read_state(&state_path);
    let ts = now();
    let mut started_at = prev["startedAt"].as_i64().unwrap_or(0);
    let project = p["cwd"]
        .as_str()
        .and_then(|c| Path::new(c).file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| prev["project"].as_str().unwrap_or("").to_string());

    let (state, label) = match event.as_str() {
        "prompt" => {
            started_at = ts;
            ("thinking", "Thinking…".to_string())
        }
        "pre" => {
            let tool = p["tool_name"].as_str().unwrap_or("");
            if started_at == 0 {
                started_at = ts;
            }
            ("tool", tool_label(tool).to_string())
        }
        "post" => {
            if started_at == 0 {
                started_at = ts;
            }
            ("thinking", "Thinking…".to_string())
        }
        "notify" => {
            // Only a permission prompt drives the pet; other notifications
            // (esp. "waiting for your input") must not park it on a stale state.
            let msg = p["message"].as_str().unwrap_or("").to_lowercase();
            let is_perm = p["notification_type"].as_str() == Some("permission_prompt")
                || msg.contains("permission")
                || msg.contains("approve")
                || msg.contains("allow");
            if !is_perm {
                return;
            }
            started_at = 0;
            ("permission", "Awaiting permission".to_string())
        }
        "permreq" => {
            started_at = 0;
            ("permission", "Awaiting permission".to_string())
        }
        "stop" => {
            started_at = 0;
            ("done", "Done".to_string())
        }
        _ => return,
    };

    // Host app for double-click activation: the hook inherits the terminal's env;
    // no TERM_PROGRAM means a non-terminal surface (Claude Desktop).
    let host = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "Claude".to_string());

    let out = json!({
        "state": state,
        "label": label,
        "tool": p["tool_name"].as_str().unwrap_or(""),
        "project": project,
        "sessionId": p["session_id"].as_str().unwrap_or(""),
        "transcript": p["transcript_path"].as_str().unwrap_or_else(|| prev["transcript"].as_str().unwrap_or("")),
        "host": host,
        "startedAt": started_at,
        "ts": ts,
    });
    write_atomic(&state_path, &out);
}
