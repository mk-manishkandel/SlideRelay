//! "Add to PowerPoint": installs the SlideRelay add-in's manifest so the
//! Broadcast button appears on PowerPoint's Home tab. The add-in itself is
//! web content served from GitHub Pages; the manifest just tells PowerPoint
//! where to find it.
//!
//! - macOS: copy the manifest into PowerPoint's "wef" folder (macOS asks the
//!   presenter once to let SlideRelay access PowerPoint's data).
//! - Windows: save the manifest in SlideRelay's data folder and register it
//!   under HKCU\Software\Microsoft\Office\16.0\WEF\Developer (per user, no
//!   admin rights needed).
//!
//! Organisations can deploy the add-in centrally instead (docs/APPSOURCE.md).

use std::path::{Path, PathBuf};

/// The production add-in manifest, built into the app.
pub const MANIFEST: &str = include_str!("../../../addin/manifest.xml");
/// Must match <Id> in addin/manifest.xml.
pub const ADDIN_ID: &str = "7fb1353c-bb37-4bf0-8739-9517d4ca3377";
const FILE_NAME: &str = "SlideRelay.xml";

#[cfg(target_os = "macos")]
fn wef_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Containers/com.microsoft.Powerpoint/Data/Documents/wef"))
}

/// True if PowerPoint will load the SlideRelay add-in.
pub fn is_installed(data_dir: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = data_dir;
        let Some(dir) = wef_dir() else { return false };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            std::fs::read_to_string(e.path())
                .map(|text| text.contains(ADDIN_ID))
                .unwrap_or(false)
        })
    }
    #[cfg(windows)]
    {
        windows_registry::registered_path()
            .map(|p| p == manifest_path(data_dir) && p.exists())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = data_dir;
        false
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn manifest_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// Installs (or refreshes) the manifest. PowerPoint picks it up the next
/// time it starts.
pub fn install(data_dir: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = data_dir;
        let dir = wef_dir().ok_or_else(|| anyhow::anyhow!("no home folder"))?;
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(FILE_NAME), MANIFEST)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        std::fs::create_dir_all(data_dir)?;
        let path = manifest_path(data_dir);
        std::fs::write(&path, MANIFEST)?;
        windows_registry::register(&path)?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = data_dir;
        anyhow::bail!("not supported on this system")
    }
}

#[cfg(windows)]
mod windows_registry {
    use super::ADDIN_ID;
    use std::path::{Path, PathBuf};
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const KEY: &str = r"Software\Microsoft\Office\16.0\WEF\Developer";

    pub fn register(manifest: &Path) -> anyhow::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(KEY)?;
        key.set_value(ADDIN_ID, &manifest.to_string_lossy().to_string())?;
        Ok(())
    }

    pub fn registered_path() -> Option<PathBuf> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(KEY).ok()?;
        let value: String = key.get_value(ADDIN_ID).ok()?;
        Some(PathBuf::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_manifest_is_the_production_one() {
        assert!(MANIFEST.contains(&format!("<Id>{ADDIN_ID}</Id>")));
        assert!(MANIFEST.contains("https://mk-manishkandel.github.io/SlideRelay/"));
    }
}
