//! Window management for Thunderbird on KDE Plasma 6 Wayland

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct WindowManager {
    tb_command: String,
    /// Tracks whether TB window is currently shown (visible) or hidden
    visible: Arc<AtomicBool>,
}

impl WindowManager {
    pub fn new(thunderbird_command: &str, visible: Arc<AtomicBool>) -> Self {
        Self {
            tb_command: thunderbird_command.to_string(),
            visible,
        }
    }

    pub fn is_thunderbird_running(&self) -> bool {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(cmdline) = std::fs::read_to_string(path.join("cmdline")) {
                    // cmdline has null-separated args; check the first arg (the binary)
                    let exe = cmdline.split('\0').next().unwrap_or("");
                    if exe.contains("/thunderbird") || exe == "thunderbird" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Spawn Thunderbird and return the process handle (for event-driven monitoring)
    pub async fn spawn_thunderbird(&self) -> Result<tokio::process::Child, Box<dyn Error>> {
        tracing::info!("Starting Thunderbird: {}", self.tb_command);
        let child = tokio::process::Command::new(&self.tb_command).spawn()?;
        Ok(child)
    }

    /// Start Thunderbird if not already running (fire-and-forget, no handle returned)
    pub async fn ensure_thunderbird_running(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_thunderbird_running() {
            tracing::info!("Starting Thunderbird: {}", self.tb_command);
            tokio::process::Command::new(&self.tb_command)
                .spawn()?;
        }
        Ok(())
    }

    /// Wait for TB window to appear in KWin (polls rapidly)
    async fn wait_for_window(&self) -> bool {
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if self.has_kwin_window().await {
                return true;
            }
        }
        false
    }

    /// Check if KWin has a Thunderbird window (cheap /proc check for >1 thread as proxy)
    async fn has_kwin_window(&self) -> bool {
        // TB creates its main window shortly after multiple threads are running
        // A more reliable check: see if KWin knows about the window
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(cmdline) = std::fs::read_to_string(path.join("cmdline")) {
                    let exe = cmdline.split('\0').next().unwrap_or("");
                    if exe.contains("/thunderbird") || exe == "thunderbird" {
                        // Check if it has enough threads (window created = many threads)
                        if let Ok(tasks) = std::fs::read_dir(path.join("task")) {
                            return tasks.count() > 5;
                        }
                    }
                }
            }
        }
        false
    }

    /// Start TB hidden, returning the Child handle for event-driven monitoring.
    /// Relies on the persistent KWin auto-hide listener (installed at startup) to hide
    /// the window the instant it appears — fully event-driven, zero polling.
    pub async fn start_hidden(&self) -> Result<tokio::process::Child, Box<dyn Error>> {
        let child = self.spawn_thunderbird().await?;
        self.visible.store(false, Ordering::Relaxed);
        tracing::info!("Thunderbird spawned (auto-hide listener will catch the window)");
        Ok(child)
    }

    /// Toggle TB window: if visible -> hide, if hidden -> show, if not running -> start+show
    pub async fn toggle_visibility(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_thunderbird_running() {
            self.ensure_thunderbird_running().await?;
            self.wait_for_window().await;
            crate::kwin_script::show_thunderbird_window().await?;
            self.visible.store(true, Ordering::Relaxed);
            return Ok(());
        }

        if self.visible.load(Ordering::Relaxed) {
            crate::kwin_script::hide_thunderbird_window().await?;
            self.visible.store(false, Ordering::Relaxed);
        } else {
            crate::kwin_script::show_thunderbird_window().await?;
            self.visible.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let visible = Arc::new(AtomicBool::new(false));
        let wm = WindowManager::new("thunderbird", visible);
        assert_eq!(wm.tb_command, "thunderbird");
    }
}
