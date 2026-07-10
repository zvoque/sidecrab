//! Shared filesystem locations. The hook binary (writer) and the app (reader)
//! must agree on these; CLAWD_PET_HOME overrides for tests.

use std::path::PathBuf;

pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("CLAWD_PET_HOME") {
        return PathBuf::from(h);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("clawd-pet")
}

pub fn state_path() -> PathBuf {
    home().join("state.json")
}

pub fn claude_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("settings.json")
}
