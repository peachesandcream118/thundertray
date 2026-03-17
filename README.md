# ThunderTray

A lightweight, native system tray application for Thunderbird on KDE Plasma 6 Wayland. Think [birdtray](https://github.com/gyunaev/birdtray), but vibecoded in Rust for modern Wayland desktops.

## Features

- **System tray icon** with live unread message count badge
- **Click to show/hide** Thunderbird (via KWin D-Bus scripting — no X11 needed)
- **Auto-start & auto-hide** — launches Thunderbird hidden on login
- **Event-driven watchdog** — instantly restarts Thunderbird if it exits
- **Auto-detect** Thunderbird profile and INBOX folders
- **Settings GUI** via kdialog (right-click tray → Settings)
- **CLI subcommands** — `install`, `uninstall`, `settings`, `status`

## Requirements

- KDE Plasma 6 (Wayland) with KWin
- Thunderbird
- D-Bus session bus
- `kdialog` (optional, for settings GUI — pre-installed on KDE)

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
thundertray install      Install systemd service + autostart entry, enable and start
thundertray uninstall    Stop service, remove all files (service, autostart, config, temp)
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

## Architecture

```
main.rs            — CLI dispatch + daemon startup
├── cli.rs         — Clap CLI definition
├── installer.rs   — Install/uninstall/status subcommands
├── settings_gui.rs — kdialog/zenity settings GUI
├── config.rs      — TOML config + Thunderbird profile detection
├── mork.rs        — Mork .msf parser + mbox fallback for unread counts
├── icon.rs        — 24×24 tray icon rendering with badge (tiny-skia)
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
