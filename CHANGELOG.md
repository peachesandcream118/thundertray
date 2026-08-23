# Changelog

## 1.1.0 - 2026-08-23

### Fixed

- Detect native, Snap, and Flatpak Thunderbird profiles without exiting before the tray starts.
- Read Thunderbird's actual unread total from its exact `folderCache.json` entry instead of treating the biff-only `numNewMsgs` value as unread.
- Keep `99+` and other badge text within the icon canvas.
- Respect `auto_start_thunderbird = false` and stop killing Thunderbird when ThunderTray exits.
- Accept command arguments such as `snap run thunderbird` without invoking a shell.
- Prevent zero-second polling, malformed colors, and unsafe predictable temporary files.
- Preserve user configuration during uninstall unless `--purge` is requested.

### Changed

- Install the executable at `~/.local/bin/thundertray`, and restart an active service on update, so moves and upgrades take effect reliably.
- Pause the watchdog after repeated rapid Thunderbird exits.
- Add continuous formatting, lint, test, release-build, and RustSec checks.
