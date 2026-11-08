//! Teacher settings, saved as JSON in the app's config folder.
//!
//! Missing or broken settings files fall back to defaults, so a bad file can
//! never stop the app from starting.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// Smaller frames for slow Wi-Fi.
    Low,
    Balanced,
    /// Sharpest text, more data.
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarPosition {
    TopCenter,
    BottomCenter,
    TopRight,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Port students connect to. 0 = choose automatically (80, then 8080, ...).
    pub port: u16,
    pub quality: Quality,
    /// Maximum frames per second sent to students.
    pub max_fps: u8,
    /// Network adapter IP to advertise; `None` = pick automatically.
    pub lan_ip: Option<String>,
    /// Optional PIN students must enter (digits only).
    pub pin: Option<String>,
    pub toolbar_position: ToolbarPosition,
    pub launch_at_login: bool,
    /// Set once the setup wizard has been completed.
    pub setup_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 0,
            quality: Quality::Balanced,
            max_fps: 15,
            lan_ip: None,
            pin: None,
            toolbar_position: ToolbarPosition::TopCenter,
            launch_at_login: false,
            setup_done: false,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("settings file is invalid, using defaults: {e}");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Paths of SlideRelay's own files.
#[derive(Debug, Clone)]
pub struct Paths {
    pub settings: PathBuf,
    pub tokens: PathBuf,
    /// macOS: local HTTPS certificate for the add-in connection.
    pub tls: PathBuf,
    pub logs: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_broken_file_gives_defaults() {
        let dir =
            std::env::temp_dir().join(format!("sliderelay-cfg-{}", crate::pairing::random_hex(4)));
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn round_trip_and_partial_files() {
        let dir =
            std::env::temp_dir().join(format!("sliderelay-cfg-{}", crate::pairing::random_hex(4)));
        let path = dir.join("settings.json");
        let s = Settings {
            port: 9000,
            pin: Some("1234".into()),
            ..Settings::default()
        };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        // Older files without newer fields still load.
        std::fs::write(&path, r#"{"port": 8081}"#).unwrap();
        let partial = Settings::load(&path);
        assert_eq!(partial.port, 8081);
        assert_eq!(partial.max_fps, 15);
        let _ = std::fs::remove_dir_all(dir);
    }
}
