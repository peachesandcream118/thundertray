use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "thunderbird.rs"]
pub mod thunderbird;

pub use thunderbird::{detect_thunderbird_profile, detect_thunderbird_profile_for_command};

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub thunderbird_command: String,
    pub auto_start_thunderbird: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub badge_color: String,
    pub badge_text_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitoringConfig {
    pub profile_path: Option<PathBuf>,
    pub poll_interval_secs: u64,
    pub folders: Vec<PathBuf>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            thunderbird_command: "thunderbird".to_string(),
            auto_start_thunderbird: true,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            badge_color: "#FF0000".to_string(),
            badge_text_color: "#FFFFFF".to_string(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            profile_path: None,
            poll_interval_secs: 5,
            folders: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration from the standard config path
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        Self::load_from_path(&config_path)
    }

    /// Save configuration to the standard config path
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        self.save_to_path(&config_path)
    }

    /// Validates values that are unsafe or unusable at runtime.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if !(1..=3600).contains(&self.monitoring.poll_interval_secs) {
            return Err(ConfigValidationError::new(format!(
                "monitoring.poll_interval_secs must be between 1 and 3600 seconds (got {})",
                self.monitoring.poll_interval_secs
            )));
        }

        if self.general.thunderbird_command.trim().is_empty() {
            return Err(ConfigValidationError::new(
                "general.thunderbird_command must not be empty",
            ));
        }

        validate_color("appearance.badge_color", &self.appearance.badge_color)?;
        validate_color(
            "appearance.badge_text_color",
            &self.appearance.badge_text_color,
        )?;

        Ok(())
    }

    fn load_from_path(config_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        match fs::read_to_string(config_path) {
            Ok(content) => {
                let config: Config = toml::from_str(&content)?;
                config.validate()?;
                Ok(config)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                config.save_to_path(config_path)?;
                Ok(config)
            }
            Err(error) => Err(Box::new(std::io::Error::new(
                error.kind(),
                format!(
                    "Could not read configuration at {}: {error}",
                    config_path.display()
                ),
            ))),
        }
    }

    fn save_to_path(&self, config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.validate()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_string = toml::to_string_pretty(self)?;
        fs::write(config_path, toml_string)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir().ok_or("Could not determine config directory")?;
        Ok(config_dir.join("thundertray/config.toml"))
    }
}

/// Error returned when configuration values are outside ThunderTray's safe range.
#[derive(Debug)]
pub struct ConfigValidationError {
    message: String,
}

impl ConfigValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigValidationError {}

fn validate_color(field: &str, color: &str) -> Result<(), ConfigValidationError> {
    let bytes = color.as_bytes();
    let is_valid_length = matches!(bytes.len(), 4 | 7);
    let is_valid = is_valid_length
        && bytes.first() == Some(&b'#')
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit());

    if is_valid {
        Ok(())
    } else {
        Err(ConfigValidationError::new(format!(
            "{field} must be #RGB or #RRGGBB hexadecimal (got {color:?})"
        )))
    }
}

/// Discover INBOX.msf files in the given Thunderbird profile
pub fn discover_inbox_msf_files(profile_path: &Path) -> Vec<PathBuf> {
    let mut inbox_files = Vec::new();

    // Check both Mail and ImapMail directories
    for mail_dir_name in &["Mail", "ImapMail"] {
        let mail_dir = profile_path.join(mail_dir_name);
        if !mail_dir.exists() {
            continue;
        }

        // Read account folders (one level deep)
        if let Ok(entries) = fs::read_dir(&mail_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        // Check for INBOX.msf in this account folder
                        let inbox_msf = entry.path().join("INBOX.msf");
                        if inbox_msf.exists() {
                            inbox_files.push(inbox_msf);
                        }
                    }
                }
            }
        }
    }

    inbox_files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_config_default_roundtrip() {
        let config = Config::default();

        // Serialize to TOML
        let toml_string = toml::to_string(&config).expect("Failed to serialize config");

        // Deserialize back
        let deserialized: Config =
            toml::from_str(&toml_string).expect("Failed to deserialize config");

        // Verify fields match
        assert_eq!(
            config.general.thunderbird_command,
            deserialized.general.thunderbird_command
        );
        assert_eq!(
            config.general.auto_start_thunderbird,
            deserialized.general.auto_start_thunderbird
        );

        assert_eq!(
            config.appearance.badge_color,
            deserialized.appearance.badge_color
        );
        assert_eq!(
            config.appearance.badge_text_color,
            deserialized.appearance.badge_text_color
        );

        assert_eq!(
            config.monitoring.profile_path,
            deserialized.monitoring.profile_path
        );
        assert_eq!(
            config.monitoring.poll_interval_secs,
            deserialized.monitoring.poll_interval_secs
        );
        assert_eq!(config.monitoring.folders, deserialized.monitoring.folders);
    }

    #[test]
    fn partial_config_uses_defaults_for_missing_fields() {
        let config: Config = toml::from_str("[general]\nauto_start_thunderbird = false\n")
            .expect("partial configuration should remain compatible");

        assert!(!config.general.auto_start_thunderbird);
        assert_eq!(config.general.thunderbird_command, "thunderbird");
        assert_eq!(config.monitoring.poll_interval_secs, 5);
        assert_eq!(config.appearance.badge_color, "#FF0000");
    }

    #[test]
    fn test_default_values() {
        let config = Config::default();

        assert_eq!(config.general.thunderbird_command, "thunderbird");
        assert!(config.general.auto_start_thunderbird);

        assert_eq!(config.appearance.badge_color, "#FF0000");
        assert_eq!(config.appearance.badge_text_color, "#FFFFFF");

        assert_eq!(config.monitoring.profile_path, None);
        assert_eq!(config.monitoring.poll_interval_secs, 5);
        assert!(config.monitoring.folders.is_empty());
    }

    #[test]
    fn validation_accepts_poll_interval_and_color_boundaries() {
        for poll_interval_secs in [1, 3600] {
            let mut config = Config::default();
            config.monitoring.poll_interval_secs = poll_interval_secs;
            config.appearance.badge_color = "#aB0".to_string();
            config.appearance.badge_text_color = "#123aBc".to_string();

            assert!(config.validate().is_ok());
        }
    }

    #[test]
    fn validation_rejects_each_unsafe_value() {
        let mut config = Config::default();
        config.monitoring.poll_interval_secs = 0;
        assert!(config
            .validate()
            .expect_err("zero poll interval must fail")
            .to_string()
            .contains("monitoring.poll_interval_secs"));

        config.monitoring.poll_interval_secs = 3601;
        assert!(config
            .validate()
            .expect_err("poll interval above the maximum must fail")
            .to_string()
            .contains("monitoring.poll_interval_secs"));

        config.monitoring.poll_interval_secs = 5;
        config.general.thunderbird_command.clear();
        assert!(config
            .validate()
            .expect_err("empty Thunderbird command must fail")
            .to_string()
            .contains("general.thunderbird_command"));

        config.general.thunderbird_command = "thunderbird".to_string();
        for color in ["FF0000", "#FF", "#FFFF", "#FFFF0000", "#GGG"] {
            config.appearance.badge_color = color.to_string();
            assert!(config
                .validate()
                .expect_err("invalid badge color must fail")
                .to_string()
                .contains("appearance.badge_color"));
        }

        config.appearance.badge_color = "#ABC".to_string();
        config.appearance.badge_text_color = "#nope".to_string();
        assert!(config
            .validate()
            .expect_err("invalid badge text color must fail")
            .to_string()
            .contains("appearance.badge_text_color"));
    }

    #[test]
    fn load_and_save_validate_before_accepting_or_writing_configuration() {
        let temp = tempdir().expect("create temporary config directory");
        let config_path = temp.path().join("nested/config.toml");
        let mut invalid = Config::default();
        invalid.monitoring.poll_interval_secs = 0;

        let save_error = invalid
            .save_to_path(&config_path)
            .expect_err("save must reject an invalid configuration");
        assert!(save_error
            .to_string()
            .contains("monitoring.poll_interval_secs"));
        assert!(!config_path.exists());

        fs::create_dir_all(config_path.parent().expect("temporary config parent"))
            .expect("create temporary config parent");
        fs::write(
            &config_path,
            toml::to_string(&invalid).expect("serialize invalid configuration"),
        )
        .expect("write invalid configuration");

        let load_error = Config::load_from_path(&config_path)
            .expect_err("load must reject an invalid configuration");
        assert!(load_error
            .to_string()
            .contains("monitoring.poll_interval_secs"));
    }
}
