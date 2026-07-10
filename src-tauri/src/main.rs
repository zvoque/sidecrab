// Prevents an extra console window on Windows in release. DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Terminal-friendly launch: typing the command detaches the pet and returns
/// the shell immediately. `--foreground` (or the daemon-child env marker)
/// runs attached — used for debugging and by launchd/brew services.
fn main() {
    let foreground = std::env::args().any(|a| a == "--foreground")
        || std::env::var_os("SIDECRAB_CHILD").is_some();
    if !foreground {
        if let Ok(exe) = std::env::current_exe() {
            let ok = std::process::Command::new(exe)
                .arg("--foreground")
                .env("SIDECRAB_CHILD", "1")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok();
            if ok {
                println!("pet is running (single instance enforced)");
                return;
            }
        }
        // Spawn failed — fall through and run attached rather than not at all.
    }
    sidecrab_lib::run()
}
