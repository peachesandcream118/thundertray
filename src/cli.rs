use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "thundertray", version, about = "System tray daemon for Thunderbird on KDE Plasma Wayland")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install ThunderTray (systemd service + autostart entry)
    Install,
    /// Uninstall ThunderTray (stop service, remove all files)
    Uninstall,
    /// Open the settings dialog
    Settings,
    /// Show ThunderTray status
    Status,
}
