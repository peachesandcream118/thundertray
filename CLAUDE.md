# ThunderTray project rules

ThunderTray is a Rust system-tray daemon for Thunderbird on KDE Plasma 6 Wayland.

## Working contract

- Keep changes focused, small, and maintainable; remove superseded code and fixtures.
- Treat reported badge-count errors as real until runtime evidence disproves them.
- Preserve native, Snap, and Flatpak Thunderbird support.
- Use Thunderbird `folderCache.json` unread totals as the primary badge source and mailbox flags only as the fallback. Do not substitute `numNewMsgs`; it is a biff/new-mail value, not the unread total.
- Keep unread aggregation overflow-safe and render counts above 99 as `99+`.
- Do not install the daemon, restart its user service, push commits, publish releases, or delete user configuration unless the user explicitly requests that action.
- Preserve unrelated user changes in a dirty worktree.

## Required checks

Run these before calling a code change complete:

```bash
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --all-features
```

For dependency changes, also run `cargo audit` when it is installed.

Runtime checks that use the real Thunderbird profile are additional evidence, not unit-test replacements. Never expose mail contents, account identifiers, tokens, or profile data in reports or fixtures.
