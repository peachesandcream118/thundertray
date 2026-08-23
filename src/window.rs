//! Window management for Thunderbird on KDE Plasma 6 Wayland

use std::error::Error;

type WindowError = Box<dyn Error + Send + Sync>;

pub struct WindowManager {
    program: String,
    args: Vec<String>,
}

impl WindowManager {
    pub fn new(thunderbird_command: &str) -> Self {
        let mut parts = shlex::split(thunderbird_command)
            .filter(|parts| !parts.is_empty())
            .unwrap_or_else(|| vec!["thunderbird".to_string()]);
        let program = parts.remove(0);
        Self {
            program,
            args: parts,
        }
    }

    pub fn is_thunderbird_running(&self) -> bool {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(cmdline) = std::fs::read_to_string(path.join("cmdline")) {
                    if process_looks_like_thunderbird(&cmdline) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Spawn Thunderbird and return the process handle (for event-driven monitoring)
    pub async fn spawn_thunderbird(&self) -> Result<tokio::process::Child, WindowError> {
        tracing::info!("Starting Thunderbird: {} {:?}", self.program, self.args);
        let child = tokio::process::Command::new(&self.program)
            .args(&self.args)
            .spawn()?;
        Ok(child)
    }

    /// Start Thunderbird if not already running (fire-and-forget, spawns reaper)
    pub async fn ensure_thunderbird_running(&self) -> Result<(), WindowError> {
        if !self.is_thunderbird_running() {
            tracing::info!("Starting Thunderbird: {} {:?}", self.program, self.args);
            let mut child = tokio::process::Command::new(&self.program)
                .args(&self.args)
                .spawn()?;
            // Reap the child in the background to prevent zombies
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
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
                    if process_looks_like_thunderbird(&cmdline) {
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
    /// Hides only the window belonging to this explicit start. There is no persistent
    /// listener, so compose windows and Thunderbird instances opened by the user are untouched.
    pub async fn start_hidden(&self) -> Result<tokio::process::Child, WindowError> {
        let child = self.spawn_thunderbird().await?;
        if self.wait_for_window().await {
            if let Err(error) = crate::kwin_script::hide_thunderbird_window().await {
                tracing::warn!("Thunderbird started, but its window could not be hidden: {error}");
            }
        } else {
            tracing::warn!("Thunderbird started, but no window appeared within 5 seconds");
        }
        Ok(child)
    }

    /// Toggle TB window: checks actual KWin state so it's always correct,
    /// even after external activation (e.g. notification click).
    pub async fn toggle_visibility(&self) -> Result<(), WindowError> {
        if !self.is_thunderbird_running() {
            self.ensure_thunderbird_running().await?;
            self.wait_for_window().await;
            crate::kwin_script::show_thunderbird_window().await?;
            return Ok(());
        }

        crate::kwin_script::toggle_thunderbird_window().await?;
        Ok(())
    }
}

fn process_looks_like_thunderbird(cmdline: &str) -> bool {
    cmdline.split('\0').any(|arg| {
        std::path::Path::new(arg)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("thunderbird"))
            || arg.contains("org.mozilla.Thunderbird")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let wm = WindowManager::new("thunderbird");
        assert_eq!(wm.program, "thunderbird");
        assert!(wm.args.is_empty());
    }

    #[test]
    fn test_command_with_arguments_is_split_without_shell() {
        let wm = WindowManager::new("snap run thunderbird");
        assert_eq!(wm.program, "snap");
        assert_eq!(wm.args, ["run", "thunderbird"]);
    }

    #[test]
    fn test_snap_process_is_detected_from_arguments() {
        assert!(process_looks_like_thunderbird(
            "/usr/bin/snap\0run\0thunderbird\0"
        ));
        assert!(process_looks_like_thunderbird(
            "/usr/bin/flatpak\0run\0org.mozilla.Thunderbird\0"
        ));
        assert!(!process_looks_like_thunderbird("/usr/bin/bash\0"));
    }
}
