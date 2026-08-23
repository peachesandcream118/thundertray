use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "thundertray",
    version,
    about = "System tray daemon for Thunderbird on KDE Plasma Wayland"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install ThunderTray binary and systemd user service
    Install,
    /// Uninstall ThunderTray while preserving configuration
    Uninstall {
        /// Also remove ~/.config/thundertray
        #[arg(long)]
        purge: bool,
    },
    /// Open the settings dialog
    Settings,
    /// Show ThunderTray status
    Status,
}
