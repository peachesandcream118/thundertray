use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_binary_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(std::env::current_exe()?)
}

fn systemd_service_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("Could not determine config directory")?;
    Ok(config.join("systemd/user/thundertray.service"))
}

fn autostart_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("Could not determine config directory")?;
    Ok(config.join("autostart/thundertray.desktop"))
}

fn config_dir_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("Could not determine config directory")?;
    Ok(config.join("thundertray"))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = get_binary_path()?;
    let bin_str = bin_path.display();

    println!("Installing ThunderTray...");
    println!("  Binary: {}", bin_str);

    // 1. Write systemd service
    let service_path = systemd_service_path()?;
    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let service_content = format!(
        "[Unit]\n\
         Description=ThunderTray - Thunderbird system tray integration\n\
         After=graphical-session.target\n\
         PartOf=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={bin_str}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         TimeoutStopSec=5\n\
         Environment=RUST_LOG=info\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n"
    );
    fs::write(&service_path, &service_content)?;
    println!("  Wrote systemd service: {}", service_path.display());

    // 2. Create default config if missing
    let config_dir = config_dir_path()?;
    let config_file = config_dir.join("config.toml");
    if !config_file.exists() {
        let config = crate::config::Config::default();
        config.save()?;
        println!("  Created default config: {}", config_file.display());
    } else {
        println!("  Config already exists: {}", config_file.display());
    }

    // 4. Enable and start systemd service
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let status = Command::new("systemctl")
        .args(["--user", "enable", "--now", "thundertray"])
        .status()?;

    if status.success() {
        println!("  Systemd service enabled and started");
    } else {
        println!("  Warning: systemctl enable failed (you may need to start manually)");
    }

    // Remove any stale desktop autostart entry (systemd handles startup)
    let autostart = autostart_path()?;
    if autostart.exists() {
        let _ = fs::remove_file(&autostart);
        println!("  Removed stale autostart entry (systemd handles startup)");
    }

    println!("\nThunderTray installed successfully!");
    println!("It will start automatically on login.");
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    println!("Uninstalling ThunderTray...");

    // 1. Stop and disable systemd service
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "thundertray"])
        .status();
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "thundertray"])
        .status();
    println!("  Stopped and disabled systemd service");

    // 2. Remove systemd service file
    let service_path = systemd_service_path()?;
    if service_path.exists() {
        fs::remove_file(&service_path)?;
        println!("  Removed: {}", service_path.display());
    }

    // 3. Remove autostart entry
    let autostart = autostart_path()?;
    if autostart.exists() {
        fs::remove_file(&autostart)?;
        println!("  Removed: {}", autostart.display());
    }

    // 4. Reload systemd
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    // 5. Remove config directory
    let config_dir = config_dir_path()?;
    if config_dir.exists() {
        fs::remove_dir_all(&config_dir)?;
        println!("  Removed config: {}", config_dir.display());
    }

    // 6. Clean up temp files
    if let Ok(entries) = fs::read_dir("/tmp") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("thundertray_") && name.ends_with(".js") {
                    let _ = fs::remove_file(entry.path());
                    println!("  Removed temp: {}", entry.path().display());
                }
            }
        }
    }

    println!("\nThunderTray uninstalled.");
    Ok(())
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    // Check if service is active
    let service_status = Command::new("systemctl")
        .args(["--user", "is-active", "thundertray"])
        .output();

    let active = match &service_status {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim() == "active",
        Err(_) => false,
    };

    println!("ThunderTray Status");
    println!("==================");
    println!("  Service: {}", if active { "running" } else { "stopped" });

    // Check if TB is running
    let tb_running = std::fs::read_dir("/proc")
        .map(|entries| {
            entries.flatten().any(|entry| {
                std::fs::read_to_string(entry.path().join("cmdline"))
                    .map(|cmd| {
                        let exe = cmd.split('\0').next().unwrap_or("");
                        exe.contains("/thunderbird") || exe == "thunderbird"
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    println!("  Thunderbird: {}", if tb_running { "running" } else { "not running" });

    // Config path
    let config_dir = config_dir_path()?;
    let config_file = config_dir.join("config.toml");
    println!("  Config: {}", config_file.display());
    println!("  Config exists: {}", config_file.exists());

    // Service file
    let service_path = systemd_service_path()?;
    println!("  Service file: {}", service_path.display());
    println!("  Service installed: {}", service_path.exists());

    // Autostart
    let autostart = autostart_path()?;
    println!("  Autostart: {}", autostart.display());
    println!("  Autostart installed: {}", autostart.exists());

    Ok(())
}
