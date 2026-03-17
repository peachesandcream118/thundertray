use std::process::Command;

const SETTINGS_SCRIPT: &str = include_str!("settings_dialog.py");

/// Open the settings dialog — a single form window with all settings visible.
pub fn open_settings() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::Config::load()?;

    if !has_command("python3") {
        show_fallback_message();
        return Ok(());
    }

    let script_path = std::env::temp_dir().join("thundertray_settings.py");
    std::fs::write(&script_path, SETTINGS_SCRIPT)?;

    let output = Command::new("python3")
        .arg(&script_path)
        .arg(&config.general.thunderbird_command)
        .arg(config.general.auto_start_thunderbird.to_string())
        .arg(&config.appearance.badge_color)
        .arg(&config.appearance.badge_text_color)
        .arg(config.monitoring.poll_interval_secs.to_string())
        .output()?;

    let _ = std::fs::remove_file(&script_path);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.trim().lines().collect();
        if lines.len() == 5 {
            let cmd = lines[0].trim();
            if !cmd.is_empty() {
                config.general.thunderbird_command = cmd.to_string();
            }
            config.general.auto_start_thunderbird = lines[1].trim() == "true";
            let badge = lines[2].trim();
            if badge.starts_with('#') && badge.len() >= 4 {
                config.appearance.badge_color = badge.to_string();
            }
            let text_col = lines[3].trim();
            if text_col.starts_with('#') && text_col.len() >= 4 {
                config.appearance.badge_text_color = text_col.to_string();
            }
            if let Ok(secs) = lines[4].trim().parse::<u64>() {
                if secs >= 1 {
                    config.monitoring.poll_interval_secs = secs;
                }
            }
            config.save()?;
            println!("Settings saved.");
        }
    } else {
        match output.status.code() {
            Some(2) => show_fallback_message(),
            _ => println!("Settings cancelled."),
        }
    }

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
        Ok(mut child) => {
            tracing::info!("Settings dialog opened");
            // Reap in background thread to prevent zombie
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => tracing::error!("Failed to open settings dialog: {}", e),
    }
}

fn show_fallback_message() {
    let config_path = dirs::config_dir()
        .map(|d| d.join("thundertray/config.toml"))
        .unwrap_or_default();
    println!(
        "No GUI toolkit available (needs python3 with PyQt6, PySide6, PyQt5, or tkinter).\n\
         Edit the config file directly: {}",
        config_path.display()
    );
}

fn has_command(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
