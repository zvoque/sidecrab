//! Persisted pet settings: window position, size, wander toggle, hook consent.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Physical screen coords of the window's top-left; None = default bottom-right.
    pub position: Option<(i32, i32)>,
    pub size: String, // "S" | "M" | "L"
    pub wander_enabled: bool,
    /// User accepted the settings.json hook-install disclaimer.
    pub hooks_consent: bool,
    /// The first-run disclaimer was shown (regardless of answer) — never nag again.
    pub consent_asked: bool,
    /// Cosmetic hat: "none" | "top" | "chef" | "fedora" | "heli".
    pub hat: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            position: None,
            size: "M".into(),
            wander_enabled: false,
            hooks_consent: false,
            consent_asked: false,
            hat: "none".into(),
        }
    }
}

fn path() -> PathBuf {
    crate::paths::home().join("config.json")
}

pub fn load_from(p: &Path) -> Config {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_to(p: &Path, c: &Config) -> std::io::Result<()> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = p.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(c)?)?;
    std::fs::rename(&tmp, p)
}

pub fn load() -> Config {
    load_from(&path())
}

pub fn save(c: &Config) -> std::io::Result<()> {
    save_to(&path(), c)
}
