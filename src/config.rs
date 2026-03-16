use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub thunderbird_command: String,
    pub auto_start_thunderbird: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub badge_color: String,
    pub badge_text_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

        match fs::read_to_string(&config_path) {
            Ok(content) => {
                let config: Config = toml::from_str(&content)?;
                Ok(config)
            }
            Err(_) => {
                // File doesn't exist, create default
                let config = Config::default();
                config.save()?;
                Ok(config)
            }
        }
    }

    /// Save configuration to the standard config path
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_string = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_string)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not determine config directory")?;
        Ok(config_dir.join("thundertray/config.toml"))
    }
}

/// Detect the Thunderbird profile directory
pub fn detect_thunderbird_profile() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let profiles_ini = home.join(".thunderbird/profiles.ini");

    let file = fs::File::open(&profiles_ini)?;
    let reader = BufReader::new(file);

    let mut current_section = String::new();
    let mut install_default_path: Option<String> = None;
    let mut default_path: Option<String> = None;
    let mut default_is_relative: Option<bool> = None;
    let mut first_path: Option<String> = None;
    let mut first_is_relative: Option<bool> = None;

    let mut section_path: Option<String> = None;
    let mut section_is_relative: Option<bool> = None;
    let mut section_is_default = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            // Save previous section if it was a profile
            if current_section.starts_with("Profile") {
                if section_is_default && section_path.is_some() {
                    default_path = section_path.clone();
                    default_is_relative = section_is_relative;
                } else if first_path.is_none() && section_path.is_some() {
                    first_path = section_path.clone();
                    first_is_relative = section_is_relative;
                }
            }

            // Start new section
            current_section = line[1..line.len() - 1].to_string();
            section_path = None;
            section_is_relative = None;
            section_is_default = false;
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if current_section.starts_with("Profile") {
                match key {
                    "Path" => section_path = Some(value.to_string()),
                    "IsRelative" => section_is_relative = Some(value == "1"),
                    "Default" => section_is_default = value == "1",
                    _ => {}
                }
            } else if current_section.starts_with("Install") && key == "Default" {
                // [Install*] sections specify the actual active profile path
                install_default_path = Some(value.to_string());
            }
        }
    }

    // Check the last section
    if current_section.starts_with("Profile") {
        if section_is_default && section_path.is_some() {
            default_path = section_path;
            default_is_relative = section_is_relative;
        } else if first_path.is_none() && section_path.is_some() {
            first_path = section_path;
            first_is_relative = section_is_relative;
        }
    }

    // Priority: Install section default > Profile Default=1 > first profile
    let tb_dir = home.join(".thunderbird");
    if let Some(install_path) = install_default_path {
        let candidate = tb_dir.join(&install_path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let (path, is_relative) = if let Some(path) = default_path {
        (path, default_is_relative.unwrap_or(true))
    } else if let Some(path) = first_path {
        (path, first_is_relative.unwrap_or(true))
    } else {
        return Err("No Thunderbird profile found".into());
    };

    let profile_path = if is_relative {
        tb_dir.join(path)
    } else {
        PathBuf::from(path)
    };

    Ok(profile_path)
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

    #[test]
    fn test_config_default_roundtrip() {
        let config = Config::default();

        // Serialize to TOML
        let toml_string = toml::to_string(&config).expect("Failed to serialize config");

        // Deserialize back
        let deserialized: Config = toml::from_str(&toml_string).expect("Failed to deserialize config");

        // Verify fields match
        assert_eq!(config.general.thunderbird_command, deserialized.general.thunderbird_command);
        assert_eq!(config.general.auto_start_thunderbird, deserialized.general.auto_start_thunderbird);

        assert_eq!(config.appearance.badge_color, deserialized.appearance.badge_color);
        assert_eq!(config.appearance.badge_text_color, deserialized.appearance.badge_text_color);

        assert_eq!(config.monitoring.profile_path, deserialized.monitoring.profile_path);
        assert_eq!(config.monitoring.poll_interval_secs, deserialized.monitoring.poll_interval_secs);
        assert_eq!(config.monitoring.folders, deserialized.monitoring.folders);
    }

    #[test]
    fn test_default_values() {
        let config = Config::default();

        assert_eq!(config.general.thunderbird_command, "thunderbird");
        assert_eq!(config.general.auto_start_thunderbird, true);

        assert_eq!(config.appearance.badge_color, "#FF0000");
        assert_eq!(config.appearance.badge_text_color, "#FFFFFF");

        assert_eq!(config.monitoring.profile_path, None);
        assert_eq!(config.monitoring.poll_interval_secs, 5);
        assert!(config.monitoring.folders.is_empty());
    }
}
