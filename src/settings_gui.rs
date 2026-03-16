use std::process::Command;

/// Open the settings dialog. Uses kdialog (KDE) with zenity as fallback.
pub fn open_settings() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::Config::load()?;

    if has_command("kdialog") {
        run_kdialog_settings(&mut config)?;
    } else if has_command("zenity") {
        run_zenity_settings(&mut config)?;
    } else {
        let config_path = dirs::config_dir()
            .map(|d| d.join("thundertray/config.toml"))
            .unwrap_or_default();
        println!("No GUI dialog tool found (kdialog or zenity).");
        println!("Edit the config file directly: {}", config_path.display());
        return Ok(());
    }

    config.save()?;
    println!("Settings saved.");
    Ok(())
}

/// Open settings from the tray (spawns as detached child process so it doesn't block)
pub fn open_settings_detached() {
    let bin = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Could not determine binary path for settings: {}", e);
            return;
        }
    };
    match Command::new(bin).arg("settings").spawn() {
        Ok(_) => tracing::info!("Settings dialog opened"),
        Err(e) => tracing::error!("Failed to open settings dialog: {}", e),
    }
}

fn has_command(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_kdialog_settings(config: &mut crate::config::Config) -> Result<(), Box<dyn std::error::Error>> {
    // Thunderbird command
    if let Ok(output) = Command::new("kdialog")
        .args([
            "--title", "ThunderTray Settings",
            "--inputbox", "Thunderbird command (binary name or full path):",
            &config.general.thunderbird_command,
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() {
                config.general.thunderbird_command = val;
            }
        } else {
            // User pressed Cancel
            println!("Settings cancelled.");
            return Ok(());
        }
    }

    // Auto-start Thunderbird
    let auto_start_result = Command::new("kdialog")
        .args([
            "--title", "ThunderTray Settings",
            "--yesno", "Auto-start Thunderbird when ThunderTray starts?",
        ])
        .status();
    if let Ok(status) = auto_start_result {
        // kdialog: 0 = Yes, 1 = No, other = Cancel
        if status.code() == Some(0) {
            config.general.auto_start_thunderbird = true;
        } else if status.code() == Some(1) {
            config.general.auto_start_thunderbird = false;
        }
    }

    // Badge color
    if let Ok(output) = Command::new("kdialog")
        .args([
            "--title", "ThunderTray Settings",
            "--getcolor",
            "--default", &config.appearance.badge_color,
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() {
                config.appearance.badge_color = val;
            }
        }
    }

    // Badge text color
    if let Ok(output) = Command::new("kdialog")
        .args([
            "--title", "ThunderTray Settings",
            "--getcolor",
            "--default", &config.appearance.badge_text_color,
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() {
                config.appearance.badge_text_color = val;
            }
        }
    }

    // Poll interval
    if let Ok(output) = Command::new("kdialog")
        .args([
            "--title", "ThunderTray Settings",
            "--inputbox", "Mail check interval (seconds):",
            &config.monitoring.poll_interval_secs.to_string(),
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(secs) = val.parse::<u64>() {
                if secs >= 1 {
                    config.monitoring.poll_interval_secs = secs;
                }
            }
        }
    }

    Ok(())
}

fn run_zenity_settings(config: &mut crate::config::Config) -> Result<(), Box<dyn std::error::Error>> {
    // Zenity forms approach — simpler but functional
    if let Ok(output) = Command::new("zenity")
        .args([
            "--forms",
            "--title=ThunderTray Settings",
            "--text=Configure ThunderTray",
            "--add-entry=Thunderbird command",
            "--add-entry=Poll interval (seconds)",
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<&str> = val.split('|').collect();
            if let Some(cmd) = parts.first() {
                if !cmd.is_empty() {
                    config.general.thunderbird_command = cmd.to_string();
                }
            }
            if let Some(interval) = parts.get(1) {
                if let Ok(secs) = interval.parse::<u64>() {
                    if secs >= 1 {
                        config.monitoring.poll_interval_secs = secs;
                    }
                }
            }
        }
    }

    // Color picker for badge
    if let Ok(output) = Command::new("zenity")
        .args([
            "--color-selection",
            "--title=Badge Color",
            &format!("--color={}", config.appearance.badge_color),
        ])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() {
                config.appearance.badge_color = val;
            }
        }
    }

    Ok(())
}
