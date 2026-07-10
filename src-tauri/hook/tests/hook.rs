// Integration tests for the sidecrab-hook binary: spawn it exactly as Claude Code
// hooks would (event arg + JSON payload on stdin) against a temp SIDECRAB_HOME.
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_sidecrab-hook");

fn run_hook(home: &Path, event: &str, payload: &str, envs: &[(&str, &str)]) {
    let mut cmd = Command::new(BIN);
    cmd.arg(event)
        .env("SIDECRAB_HOME", home)
        .env_remove("TERM_PROGRAM")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "hook exited nonzero for event {event}");
}

fn state(home: &Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(home.join("state.json")).expect("state.json");
    serde_json::from_str(&raw).unwrap()
}

fn tmp_home(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sidecrab-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn pre_bash_writes_tool_state() {
    let home = tmp_home("pre");
    run_hook(
        &home,
        "pre",
        r#"{"tool_name":"Bash","session_id":"abc","cwd":"/x/doom"}"#,
        &[],
    );
    let s = state(&home);
    assert_eq!(s["state"], "tool");
    assert_eq!(s["label"], "Running command");
    assert_eq!(s["tool"], "Bash");
    assert_eq!(s["project"], "doom");
    assert_eq!(s["sessionId"], "abc");
    assert!(s["ts"].as_i64().unwrap() > 0);
}

#[test]
fn prompt_writes_thinking() {
    let home = tmp_home("prompt");
    run_hook(&home, "prompt", r#"{"session_id":"abc","cwd":"/x/p"}"#, &[]);
    let s = state(&home);
    assert_eq!(s["state"], "thinking");
    assert_eq!(s["label"], "Thinking…");
    assert!(s["startedAt"].as_i64().unwrap() > 0);
}

#[test]
fn unknown_tool_gets_generic_label() {
    let home = tmp_home("mcp");
    run_hook(
        &home,
        "pre",
        r#"{"tool_name":"mcp__server__thing","session_id":"abc","cwd":"/x/p"}"#,
        &[],
    );
    let s = state(&home);
    assert_eq!(s["state"], "tool");
    assert_eq!(s["label"], "Using tool");
}

#[test]
fn notify_non_permission_is_ignored() {
    let home = tmp_home("notify-idle");
    run_hook(
        &home,
        "notify",
        r#"{"session_id":"abc","message":"Claude is waiting for your input"}"#,
        &[],
    );
    assert!(!home.join("state.json").exists(), "no state should be written");
}

#[test]
fn notify_permission_writes_permission() {
    let home = tmp_home("notify-perm");
    run_hook(
        &home,
        "notify",
        r#"{"session_id":"abc","message":"Claude needs your permission to use Bash"}"#,
        &[],
    );
    let s = state(&home);
    assert_eq!(s["state"], "permission");
    assert_eq!(s["label"], "Awaiting permission");
}

#[test]
fn stop_writes_done() {
    let home = tmp_home("stop");
    run_hook(&home, "stop", r#"{"session_id":"abc"}"#, &[]);
    let s = state(&home);
    assert_eq!(s["state"], "done");
    assert_eq!(s["startedAt"], 0);
}

#[test]
fn start_registers_session_and_records_host() {
    let home = tmp_home("start");
    run_hook(
        &home,
        "start",
        r#"{"session_id":"abc"}"#,
        &[("TERM_PROGRAM", "iTerm.app")],
    );
    assert!(home.join("sessions.d").join("abc").exists());
    // A subsequent state write carries the host from the hook's environment.
    run_hook(
        &home,
        "prompt",
        r#"{"session_id":"abc","cwd":"/x/p"}"#,
        &[("TERM_PROGRAM", "iTerm.app")],
    );
    assert_eq!(state(&home)["host"], "iTerm.app");
}

#[test]
fn missing_term_program_defaults_to_claude_host() {
    let home = tmp_home("host-default");
    run_hook(&home, "prompt", r#"{"session_id":"abc","cwd":"/x/p"}"#, &[]);
    assert_eq!(state(&home)["host"], "Claude");
}

#[test]
fn end_removes_session_and_clears_own_stale_state() {
    let home = tmp_home("end-own");
    run_hook(&home, "start", r#"{"session_id":"abc"}"#, &[]);
    run_hook(
        &home,
        "pre",
        r#"{"tool_name":"Bash","session_id":"abc","cwd":"/x/p"}"#,
        &[],
    );
    run_hook(&home, "end", r#"{"session_id":"abc"}"#, &[]);
    assert!(!home.join("sessions.d").join("abc").exists());
    // Frozen "tool" state owned by the ending session resets to idle.
    assert_eq!(state(&home)["state"], "idle");
}

#[test]
fn end_of_other_session_leaves_live_state_alone() {
    let home = tmp_home("end-other");
    run_hook(
        &home,
        "pre",
        r#"{"tool_name":"Bash","session_id":"abc","cwd":"/x/p"}"#,
        &[],
    );
    run_hook(&home, "end", r#"{"session_id":"zzz"}"#, &[]);
    // Live turn owned by "abc" must survive another session's end.
    assert_eq!(state(&home)["state"], "tool");
}
