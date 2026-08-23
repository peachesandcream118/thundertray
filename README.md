# ThunderTray

A lightweight system tray application for Thunderbird on KDE Plasma 6 Wayland, written in Rust.

## Features

- **System tray icon** with live unread message count badge
- **Click to show/hide** Thunderbird (via KWin D-Bus scripting — no X11 needed)
- **Auto-start & auto-hide** — launches Thunderbird hidden on login
- **Bounded watchdog** — restarts Thunderbird after an unexpected exit and pauses after repeated failures
- **Auto-detect** native, Snap, and Flatpak Thunderbird profiles and INBOX folders
- **Settings GUI** via Python with PyQt/PySide or tkinter (right-click tray → Settings)
- **CLI subcommands** — `install`, `uninstall`, `settings`, `status`

## Requirements

- KDE Plasma 6 (Wayland) with KWin
- Thunderbird
- D-Bus session bus
- Python 3 with PyQt6, PySide6, PyQt5, or tkinter (optional, for the settings GUI)

## Quick Start

1. Download the latest `thundertray` binary from [Releases](https://github.com/peachesandcream118/thundertray/releases)
2. Make it executable and install:

```bash
chmod +x thundertray
./thundertray install
```

That's it — ThunderTray is now running in your system tray and will start automatically on login.

To remove it later:

```bash
thundertray uninstall
```

## CLI Usage

```
thundertray              Run the tray daemon (default)
thundertray install      Install the stable binary and user service, then enable and start it
thundertray uninstall    Stop the service and remove installed program files; keep config
thundertray uninstall --purge
                         Also remove the saved configuration
thundertray settings     Open the settings dialog
thundertray status       Show service and Thunderbird status
```

## Configuration

Config is auto-created at `~/.config/thundertray/config.toml`:

```toml
[general]
thunderbird_command = "thunderbird"
auto_start_thunderbird = true

[appearance]
badge_color = "#FF0000"
badge_text_color = "#FFFFFF"

[monitoring]
poll_interval_secs = 5
# profile_path is auto-detected if not set
# folders = [] means auto-discover INBOX.msf files
```

You can also edit settings via the GUI: right-click the tray icon → **Settings**, or run `thundertray settings`.

If Thunderbird is installed as a Snap and `thunderbird` is not available to the user service,
set `thunderbird_command = "snap run thunderbird"`. Flatpak users can set
`thunderbird_command = "flatpak run org.mozilla.Thunderbird"`.

## Architecture

```
main.rs            — CLI dispatch + daemon startup
├── cli.rs         — Clap CLI definition
├── installer.rs   — Install/uninstall/status subcommands
├── settings_gui.rs — optional Python settings GUI
├── config.rs      — TOML configuration and validation
├── thunderbird.rs — native/Snap/Flatpak profile detection
├── mork.rs        — Thunderbird unread cache + mbox fallback
├── icon.rs        — tray icon rendering with unread badge (tiny-skia)
├── window.rs      — Thunderbird process management
├── kwin_script.rs — KWin D-Bus scripting for window show/hide
├── watcher.rs     — Mail file monitoring
└── tray.rs        — SNI system tray integration (ksni)
```

## Building from Source

```bash
git clone https://github.com/peachesandcream118/thundertray.git
cd thundertray
cargo build --release
# Binary at target/release/thundertray
```

## License

MIT — see [LICENSE](LICENSE) for details.
