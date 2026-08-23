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
        Some(cli::Command::Uninstall { purge }) => return installer::uninstall(purge),
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
    let mut cfg = match config::Config::load() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(
                "Could not load ThunderTray configuration ({error}); using safe defaults without overwriting the file"
            );
            config::Config::default()
        }
    };
    info!("Config loaded");

    // Resolve Thunderbird profile path
    let profile_path = match &cfg.monitoring.profile_path {
        Some(path) => Some(path.clone()),
        None => {
            match config::detect_thunderbird_profile_for_command(&cfg.general.thunderbird_command) {
                Ok(detected) => {
                    info!("Auto-detected Thunderbird profile: {:?}", detected);
                    cfg.monitoring.profile_path = Some(detected.clone());
                    Some(detected)
                }
                Err(error) => {
                    tracing::error!(
                    "No Thunderbird profile found ({error}). The tray will keep running; set monitoring.profile_path in the config after Thunderbird creates a profile."
                );
                    None
                }
            }
        }
    };

    // Discover .msf files to monitor
    let msf_files = if cfg.monitoring.folders.is_empty() {
        let discovered = profile_path
            .as_deref()
            .map(config::discover_inbox_msf_files)
            .unwrap_or_default();
        info!("Discovered {} INBOX.msf files", discovered.len());
        discovered
    } else {
        cfg.monitoring.folders.clone()
    };

    if msf_files.is_empty() {
        tracing::warn!("No .msf files found to monitor. Tray will show 0 unread.");
    }

    // Run tray first; its watchdog starts Thunderbird afterwards so launch failures never
    // prevent the user from getting a tray icon and a recoverable process.
    tray::run_tray(cfg, msf_files).await?;

    Ok(())
}
