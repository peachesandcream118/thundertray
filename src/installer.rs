use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn home_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    dirs::home_dir().ok_or_else(|| "Could not determine home directory".into())
}

fn stable_binary_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home_dir()?.join(".local/bin/thundertray"))
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

fn run_systemctl(args: &[&str]) -> Result<ExitStatus, Box<dyn std::error::Error>> {
    Ok(Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()?)
}

fn require_success(operation: &str, status: ExitStatus) -> Result<(), Box<dyn std::error::Error>> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{operation} failed with {status}").into())
    }
}

fn systemd_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::current_exe()?;
    let installed_binary = stable_binary_path()?;

    println!("Installing ThunderTray...");
    if source != installed_binary {
        if let Some(parent) = installed_binary.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &installed_binary)?;
        fs::set_permissions(&installed_binary, fs::Permissions::from_mode(0o755))?;
        println!("  Installed binary: {}", installed_binary.display());
    } else {
        println!("  Binary already installed: {}", installed_binary.display());
    }

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
         ExecStart={}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         Environment=RUST_LOG=info\n\
         Environment=PATH=/usr/local/bin:/usr/bin:/bin:/var/lib/flatpak/exports/bin:/snap/bin\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        systemd_quote(&installed_binary)
    );
    fs::write(&service_path, service_content)?;
    println!("  Wrote user service: {}", service_path.display());

    let config_file = config_dir_path()?.join("config.toml");
    if !config_file.exists() {
        crate::config::Config::default().save()?;
        println!("  Created default config: {}", config_file.display());
    } else {
        crate::config::Config::load()?;
        println!("  Kept existing valid config: {}", config_file.display());
    }

    let stale_autostart = autostart_path()?;
    if stale_autostart.exists() {
        fs::remove_file(&stale_autostart)?;
        println!("  Removed obsolete desktop autostart entry");
    }

    require_success(
        "systemd user daemon reload",
        run_systemctl(&["daemon-reload"])?,
    )?;
    let was_active = run_systemctl(&["is-active", "--quiet", "thundertray.service"])
        .is_ok_and(|status| status.success());
    require_success(
        "enabling ThunderTray at login",
        run_systemctl(&["enable", "thundertray.service"])?,
    )?;
    let activation = if was_active {
        run_systemctl(&["restart", "thundertray.service"])?
    } else {
        run_systemctl(&["start", "thundertray.service"])?
    };
    require_success(
        if was_active {
            "restarting ThunderTray after update"
        } else {
            "starting ThunderTray"
        },
        activation,
    )?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    require_success(
        "ThunderTray service health check",
        run_systemctl(&["is-active", "--quiet", "thundertray.service"])?,
    )?;

    println!("ThunderTray is running. Look for the envelope in the system tray.");
    Ok(())
}

pub fn uninstall(purge: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Uninstalling ThunderTray...");

    let stopped = run_systemctl(&["disable", "--now", "thundertray.service"])
        .is_ok_and(|status| status.success());
    if !stopped
        && run_systemctl(&["is-active", "--quiet", "thundertray.service"])
            .is_ok_and(|status| status.success())
    {
        return Err(
            "Could not stop the running ThunderTray service; installed files were kept".into(),
        );
    }

    let service_path = systemd_service_path()?;
    if service_path.exists() {
        fs::remove_file(&service_path)?;
        println!("  Removed service: {}", service_path.display());
    }

    let stale_autostart = autostart_path()?;
    if stale_autostart.exists() {
        fs::remove_file(&stale_autostart)?;
        println!("  Removed obsolete autostart entry");
    }

    let _ = run_systemctl(&["daemon-reload"]);

    let installed_binary = stable_binary_path()?;
    if installed_binary.exists() {
        fs::remove_file(&installed_binary)?;
        println!("  Removed binary: {}", installed_binary.display());
    }

    let config_dir = config_dir_path()?;
    if purge && config_dir.exists() {
        fs::remove_dir_all(&config_dir)?;
        println!("  Purged config: {}", config_dir.display());
    } else if config_dir.exists() {
        println!("  Kept config: {}", config_dir.display());
    }

    println!("ThunderTray uninstalled. Thunderbird was left running.");
    Ok(())
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let active = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "thundertray.service"])
        .output()
        .is_ok_and(|output| output.status.success());
    let service_path = systemd_service_path()?;
    let installed_binary = stable_binary_path()?;
    let config_file = config_dir_path()?.join("config.toml");
    let thunderbird_running =
        crate::window::WindowManager::new("thunderbird").is_thunderbird_running();

    println!("ThunderTray status");
    println!("  Service: {}", if active { "running" } else { "stopped" });
    println!(
        "  Installed binary: {} ({})",
        installed_binary.display(),
        if installed_binary.exists() {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "  Service file: {} ({})",
        service_path.display(),
        if service_path.exists() {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "  Thunderbird: {}",
        if thunderbird_running {
            "running"
        } else {
            "not running"
        }
    );
    println!(
        "  Config: {} ({})",
        config_file.display(),
        if config_file.exists() {
            "present"
        } else {
            "missing"
        }
    );

    let profile = if config_file.exists() {
        let config = crate::config::Config::load()?;
        match config.monitoring.profile_path {
            Some(path) => Ok(path),
            None => crate::config::detect_thunderbird_profile_for_command(
                &config.general.thunderbird_command,
            ),
        }
    } else {
        crate::config::detect_thunderbird_profile()
    };

    match profile {
        Ok(profile) => {
            let inboxes = crate::config::discover_inbox_msf_files(&profile);
            let unread = crate::watcher::MailWatcher::new(inboxes.clone(), 5).get_unread_count();
            println!("  Profile: {}", profile.display());
            println!(
                "  Monitoring: {} inbox(es), {} unread",
                inboxes.len(),
                unread
            );
        }
        Err(error) => println!("  Profile: not found ({error})"),
    }

    if !active {
        println!("  Remedy: run `thundertray install` or inspect `journalctl --user -u thundertray -n 20`.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::systemd_quote;
    use std::path::Path;

    #[test]
    fn systemd_path_is_quoted_and_escaped() {
        assert_eq!(systemd_quote(Path::new("/tmp/a b")), "\"/tmp/a b\"");
        assert_eq!(systemd_quote(Path::new("/tmp/a\\b")), "\"/tmp/a\\\\b\"");
    }
}
