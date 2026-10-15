//! Load/save the persisted [`AppSettings`] (model profiles + active selection).
//!
//! Stored as JSON at `data_dir()/settings.json`. The file holds API keys in
//! plaintext (a deliberate, user-chosen trade-off), so on unix it is written
//! with `0600` permissions to keep it readable only by the owner.

use std::path::PathBuf;

use crate::shared::config;
use crate::shared::error::AppResult;
use crate::shared::models::AppSettings;

/// Path to the settings file: `<data_dir>/settings.json`.
fn settings_path() -> AppResult<PathBuf> {
    Ok(config::data_dir()?.join("settings.json"))
}

/// Read settings from disk, returning defaults when the file does not exist yet.
/// A malformed file surfaces as an error rather than silently resetting.
pub fn load() -> AppResult<AppSettings> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let settings = serde_json::from_str(&raw)?;
    Ok(settings)
}

/// Persist settings, creating the data directory if needed. On unix the file is
/// written `0600` so plaintext API keys are not world-readable.
pub fn save(settings: &AppSettings) -> AppResult<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, json)?;
    set_owner_only(&path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only(path: &std::path::Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &std::path::Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::ModelProfile;

    /// Round-trip a settings value through JSON (the on-disk format) to guard
    /// the serde contract without touching the real data dir.
    #[test]
    fn json_round_trip_preserves_profiles() {
        let settings = AppSettings {
            profiles: vec![ModelProfile {
                id: "openai".into(),
                label: "GPT-4o".into(),
                base_url: "https://api.openai.com/v1".into(),
                model: "gpt-4o".into(),
                api_key: "sk-test".into(),
                temperature: 0.3,
            }],
            active_profile_id: Some("openai".into()),
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, back);
        // camelCase on the wire.
        assert!(json.contains("baseUrl"));
        assert!(json.contains("activeProfileId"));
    }

    #[test]
    fn missing_api_key_defaults_to_empty() {
        let json = r#"{
            "profiles": [
                {"id":"l","label":"Local","baseUrl":"http://x/v1","model":"local","temperature":0.7}
            ]
        }"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.profiles[0].api_key, "");
        assert!(s.active_profile_id.is_none());
    }
}
