mod cli;
mod config;
mod icon;
mod installer;
mod kwin_script;
mod mork;
mod settings_gui;
mod tray;
mod watcher;
mod window;

use clap::Parser;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    match cli.command {
        Some(cli::Command::Install) => return installer::install(),
        Some(cli::Command::Uninstall) => return installer::uninstall(),
        Some(cli::Command::Settings) => return settings_gui::open_settings(),
        Some(cli::Command::Status) => return installer::status(),
        None => {} // Run daemon
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("ThunderTray starting");

    // Load config (creates default if missing)
    let mut cfg = config::Config::load()?;
    info!("Config loaded");

    // Resolve Thunderbird profile path
    let profile_path = match &cfg.monitoring.profile_path {
        Some(p) => p.clone(),
        None => {
            let detected = config::detect_thunderbird_profile()?;
            info!("Auto-detected Thunderbird profile: {:?}", detected);
            cfg.monitoring.profile_path = Some(detected.clone());
            detected
        }
    };

    // Discover .msf files to monitor
    let msf_files = if cfg.monitoring.folders.is_empty() {
        let discovered = config::discover_inbox_msf_files(&profile_path);
        info!("Discovered {} INBOX.msf files", discovered.len());
        discovered
    } else {
        cfg.monitoring.folders.clone()
    };

    if msf_files.is_empty() {
        tracing::warn!("No .msf files found to monitor. Tray will show 0 unread.");
    }

    // Install persistent KWin auto-hide listener (catches new TB windows instantly)
    if let Err(e) = kwin_script::install_persistent_auto_hide().await {
        tracing::warn!("Auto-hide listener failed to install (non-fatal): {}", e);
    }

    // Start Thunderbird in background if configured
    let initial_child = if cfg.general.auto_start_thunderbird {
        let wm = window::WindowManager::new(&cfg.general.thunderbird_command);
        Some(wm.start_hidden().await?)
    } else {
        None
    };

    // Run tray (blocks until shutdown signal)
    tray::run_tray(cfg, msf_files, initial_child).await?;

    Ok(())
}
