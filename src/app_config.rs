//! Persistent application configuration.
//!
//! Settings survive restarts; they are stored as JSON in the platform config
//! directory (`~/.config/par2dialog/config.json` on Linux).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::util;

/// Application-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Dark theme enabled.
    pub dark_mode: bool,
    /// Default redundancy percent for new Create jobs.
    pub default_redundancy_percent: u64,
    /// Default processing threads (empty = auto).
    pub default_threads: String,
    /// Default hashing threads (empty = par2cmdline default).
    pub default_hash_threads: String,
    /// Default memory limit in MB (empty = par2cmdline default).
    pub default_memory_limit: String,
    /// Detect `par2` on PATH instead of using [`Self::par2_path`].
    pub auto_detect_par2: bool,
    /// Custom path to the par2 binary (used when auto-detection is off or fails).
    pub par2_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dark_mode: true,
            default_redundancy_percent: 5,
            default_threads: String::new(),
            default_hash_threads: "2".to_string(),
            default_memory_limit: String::new(),
            auto_detect_par2: true,
            par2_path: String::new(),
        }
    }
}

impl AppConfig {
    /// Load the configuration from disk, falling back to defaults when the
    /// file is missing or unreadable.
    pub fn load() -> Self {
        let Some(path) = config_file() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("Ignoring invalid config {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Persist the configuration. Best effort — failures are logged, never fatal.
    pub fn save(&self) {
        let Some(path) = config_file() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Cannot create config dir {}: {e}", parent.display());
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!("Cannot save config {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!("Cannot serialize config: {e}"),
        }
    }
}

/// Directory holding `Par2Dialog`'s configuration files.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(util::APP_NAME))
}

/// Full path of the configuration file.
#[must_use]
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}
