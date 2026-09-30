//! Per-user application settings.
//!
//! Stored as JSON in the user's own app-data folder, never in the shared
//! workspace. Nothing here is a security boundary: access control is decided
//! by filesystem permissions alone.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CairnError, Result};
use crate::fsutil::{read_optional, write_atomic};
use crate::images::ImageLimits;

pub const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    /// Name shown to others while you edit. Defaults to your Windows user name.
    pub display_name: Option<String>,
    /// Workspace opened last time.
    pub last_workspace: Option<PathBuf>,
    /// Minutes without editing activity before the "are you still there?" warning.
    pub idle_warning_minutes: u64,
    /// Minutes without editing activity before the edit lock is released.
    pub idle_release_minutes: u64,
    /// Save unpublished drafts to this computer so they survive a restart.
    pub persistent_drafts: bool,
    /// Override for the private draft folder.
    pub draft_dir: Option<PathBuf>,
    pub max_image_mb: u64,
    pub max_image_dimension: u32,
    /// "light", "dark", or "contrast".
    pub appearance: String,
    /// "normal", "large", or "larger".
    pub text_size: String,
    /// IANA timezone to show times in (for example "Asia/Manila").
    /// `None` means this computer's timezone. Stored times are always UTC.
    pub timezone: Option<String>,
    /// Paper size for PDF export: "a4" or "letter".
    pub pdf_paper: String,
    /// Browser used to make PDFs. `None` means find Microsoft Edge or Chrome.
    pub pdf_browser: Option<PathBuf>,
    /// Show words beside the editor's toolbar icons (otherwise the name
    /// appears on hover and keyboard focus).
    pub toolbar_labels: bool,
    /// "daily" to look for a new version of Cairn once a day, or "manual"
    /// to look only when the person chooses Check now.
    pub update_check: String,
    /// Proxy for reaching GitHub, as `host:port`. `None` uses Windows'
    /// proxy settings.
    pub update_proxy: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            display_name: None,
            last_workspace: None,
            idle_warning_minutes: 15,
            idle_release_minutes: 20,
            persistent_drafts: true,
            draft_dir: None,
            max_image_mb: 10,
            max_image_dimension: 8000,
            appearance: "light".into(),
            text_size: "normal".into(),
            timezone: None,
            pdf_paper: "a4".into(),
            pdf_browser: None,
            // Words beside the icons unless someone chooses icons only:
            // every button should say what it does.
            toolbar_labels: true,
            update_check: "daily".into(),
            update_proxy: None,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        let bad = |msg: &str| Err(CairnError::BadRequest(msg.into()));
        if !(1..=24 * 60).contains(&self.idle_warning_minutes) {
            return bad("The warning time must be between 1 minute and 24 hours.");
        }
        if self.idle_release_minutes <= self.idle_warning_minutes
            || self.idle_release_minutes > 24 * 60
        {
            return bad(
                "The unlock time must be later than the warning time, and at most 24 hours.",
            );
        }
        if !["light", "dark", "contrast"].contains(&self.appearance.as_str()) {
            return bad("Unknown appearance.");
        }
        if !["normal", "large", "larger"].contains(&self.text_size.as_str()) {
            return bad("Unknown text size.");
        }
        if !(1..=50).contains(&self.max_image_mb)
            || !(100..=20_000).contains(&self.max_image_dimension)
        {
            return bad("Picture limits are out of range.");
        }
        if self
            .display_name
            .as_ref()
            .is_some_and(|n| n.chars().count() > 60)
        {
            return bad("Please use a name of 60 characters or fewer.");
        }
        if !["a4", "letter"].contains(&self.pdf_paper.as_str()) {
            return bad("Unknown paper size.");
        }
        if !["daily", "manual"].contains(&self.update_check.as_str()) {
            return bad("Unknown update setting.");
        }
        if let Some(proxy) = &self.update_proxy
            && crate::update::normalize_proxy(proxy)? != *proxy
        {
            return bad("Type the proxy as address:port, for example proxy.office.local:8080.");
        }
        if let Some(zone) = &self.timezone {
            crate::timefmt::validate_zone(zone)?;
        }
        Ok(())
    }

    pub fn image_limits(&self) -> ImageLimits {
        ImageLimits {
            max_bytes: (self.max_image_mb as usize) * 1024 * 1024,
            max_dimension: self.max_image_dimension,
            ..ImageLimits::default()
        }
    }
}

/// Folder holding config and (by default) drafts. `CAIRN_HOME` overrides it,
/// which tests and portable setups use.
pub fn config_dir() -> PathBuf {
    match std::env::var("CAIRN_HOME") {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("Cairn"),
    }
}

/// Load settings, falling back to defaults for a missing or unreadable file.
/// A problem with an existing file is returned so the UI can mention it.
pub fn load(dir: &Path) -> (AppConfig, Option<String>) {
    match read_optional(&dir.join(CONFIG_FILE)) {
        Ok(None) => (AppConfig::default(), None),
        Ok(Some(bytes)) => match serde_json::from_slice::<AppConfig>(&bytes) {
            Ok(cfg) if cfg.validate().is_ok() => (cfg, None),
            _ => (
                AppConfig::default(),
                Some("Your settings file couldn't be read, so default settings are in use.".into()),
            ),
        },
        Err(err) => (
            AppConfig::default(),
            Some(format!("Your settings file couldn't be read ({err}).")),
        ),
    }
}

pub fn save(dir: &Path, cfg: &AppConfig) -> Result<()> {
    cfg.validate()?;
    std::fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec_pretty(cfg).expect("config serializes");
    write_atomic(&dir.join(CONFIG_FILE), &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_release_must_follow_warning() {
        assert!(AppConfig::default().validate().is_ok());
        let bad = AppConfig {
            idle_warning_minutes: 20,
            idle_release_minutes: 15,
            ..AppConfig::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = AppConfig {
            display_name: Some("Sam".into()),
            ..AppConfig::default()
        };
        save(dir.path(), &cfg).unwrap();
        assert_eq!(load(dir.path()).0, cfg);
    }
}
