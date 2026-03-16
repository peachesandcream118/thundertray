# ThunderTray

A lightweight, native system tray application for Thunderbird on KDE Plasma 6 Wayland. Think [birdtray](https://github.com/gyunaev/birdtray), but built from scratch in Rust for modern Wayland desktops.

## Features

- **System tray icon** with live unread message count badge
- **Click to show/hide** Thunderbird (via KWin D-Bus scripting — no X11 needed)
- **Auto-start & auto-hide** — launches Thunderbird hidden on login
- **Event-driven watchdog** — instantly restarts Thunderbird if it exits
- **Auto-detect** Thunderbird profile and INBOX folders
- **Settings GUI** via kdialog (right-click tray → Settings)
- **CLI subcommands** — `install`, `uninstall`, `settings`, `status`
- **Zero-CPU idle** — event-driven architecture, no busy polling

## Requirements

- KDE Plasma 6 (Wayland) with KWin
- Thunderbird
- D-Bus session bus
- `kdialog` (optional, for settings GUI — pre-installed on KDE)

## Quick Start

```bash
# Build and install
cargo install --path .

# Set up autostart (systemd service + desktop entry)
thundertray install

# Or just run it directly
thundertray
```

## CLI Usage

```
thundertray              Run the tray daemon (default)
thundertray install      Install systemd service + autostart entry
thundertray uninstall    Remove service + autostart (add --purge to remove config)
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
git clone <repo-url>
cd thundertray
cargo build --release
# Binary at target/release/thundertray
```

## License

MIT — see [LICENSE](LICENSE) for details.
