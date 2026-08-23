use ksni::TrayMethods;
use tracing::{error, info, warn};

struct ThunderTray {
    unread_count: u32,
    badge_color: String,
    badge_text_color: String,
    toggle_tx: tokio::sync::mpsc::Sender<()>,
    quit_tx: tokio::sync::mpsc::Sender<()>,
}

impl ksni::Tray for ThunderTray {
    fn id(&self) -> String {
        "thundertray".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn title(&self) -> String {
        if self.unread_count > 0 {
            format!("ThunderTray - {} unread", self.unread_count)
        } else {
            "ThunderTray".into()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let pixmap =
            crate::icon::render_icon(self.unread_count, &self.badge_color, &self.badge_text_color);
        vec![ksni::Icon {
            width: pixmap.width,
            height: pixmap.height,
            data: pixmap.data,
        }]
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Show/Hide Thunderbird".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.toggle_tx.try_send(());
                }),
                ..Default::default()
            }),
            ksni::MenuItem::Separator,
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("Unread: {}", self.unread_count),
                enabled: false,
                ..Default::default()
            }),
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Settings...".into(),
                activate: Box::new(|_: &mut Self| {
                    crate::settings_gui::open_settings_detached();
                }),
                ..Default::default()
            }),
            ksni::MenuItem::Separator,
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.quit_tx.try_send(());
                }),
                ..Default::default()
            }),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.toggle_tx.try_send(());
    }
}

pub async fn run_tray(
    config: crate::config::Config,
    msf_files: Vec<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Initializing ThunderTray system tray");

    let (toggle_tx, mut toggle_rx) = tokio::sync::mpsc::channel::<()>(4);
    let (quit_tx, mut quit_rx) = tokio::sync::mpsc::channel::<()>(1);

    let watcher = crate::watcher::MailWatcher::new(msf_files, config.monitoring.poll_interval_secs);
    let initial_count = watcher.get_unread_count();

    let tray = ThunderTray {
        unread_count: initial_count,
        badge_color: config.appearance.badge_color.clone(),
        badge_text_color: config.appearance.badge_text_color.clone(),
        toggle_tx,
        quit_tx,
    };

    let handle = tray.spawn().await?;

    info!("Tray service spawned");

    // Spawn toggle handler task with debouncing
    let tb_command = config.general.thunderbird_command.clone();
    tokio::spawn(async move {
        let mut last_toggle = std::time::Instant::now() - std::time::Duration::from_secs(1);
        while toggle_rx.recv().await.is_some() {
            // Debounce: ignore clicks within 500ms of last toggle
            if last_toggle.elapsed() < std::time::Duration::from_millis(500) {
                continue;
            }
            last_toggle = std::time::Instant::now();
            let wm = crate::window::WindowManager::new(&tb_command);
            if let Err(e) = wm.toggle_visibility().await {
                error!("Failed to toggle Thunderbird: {}", e);
            }
        }
    });

    let shutdown = tokio_util::sync::CancellationToken::new();
    if config.general.auto_start_thunderbird {
        spawn_watchdog(config.general.thunderbird_command.clone(), shutdown.clone());
    } else {
        info!("Thunderbird auto-start and watchdog are disabled by configuration");
    }

    // Poll loop — check unread count periodically and update tray
    // Also listens for SIGTERM/SIGINT to trigger clean shutdown
    let mut last_count = initial_count;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(
        config.monitoring.poll_interval_secs,
    ));

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let new_count = watcher.get_unread_count();
                if new_count != last_count {
                    info!("Unread count changed: {} -> {}", last_count, new_count);
                    handle.update(|tray| {
                        tray.unread_count = new_count;
                    }).await;
                    last_count = new_count;
                }
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                shutdown.cancel();
                break;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                shutdown.cancel();
                break;
            }
            _ = quit_rx.recv() => {
                info!("Quit requested from tray menu");
                shutdown.cancel();
                break;
            }
        }
    }

    handle.shutdown().await;
    info!("ThunderTray stopped");
    Ok(())
}

fn spawn_watchdog(command: String, shutdown: tokio_util::sync::CancellationToken) {
    tokio::spawn(async move {
        let wm = crate::window::WindowManager::new(&command);
        let mut recent_starts = std::collections::VecDeque::new();

        loop {
            if shutdown.is_cancelled() {
                return;
            }

            if wm.is_thunderbird_running() {
                tokio::select! {
                    _ = shutdown.cancelled() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => continue,
                }
            }

            let now = std::time::Instant::now();
            while recent_starts.front().is_some_and(|started| {
                now.duration_since(*started) > std::time::Duration::from_secs(60)
            }) {
                recent_starts.pop_front();
            }
            if recent_starts.len() >= 3 {
                error!("Thunderbird exited three times in 60 seconds; watchdog paused");
                return;
            }

            match wm.start_hidden().await {
                Ok(mut child) => {
                    recent_starts.push_back(std::time::Instant::now());
                    tokio::select! {
                        status = child.wait() => info!("Thunderbird launcher exited: {status:?}"),
                        _ = shutdown.cancelled() => {
                            info!("Watchdog stopped; Thunderbird is left running");
                            return;
                        }
                    }
                }
                Err(error) => {
                    warn!("Could not start Thunderbird: {error}; retrying in 5 seconds");
                    tokio::select! {
                        _ = shutdown.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_tray(unread_count: u32) -> ThunderTray {
        let (toggle_tx, _rx) = tokio::sync::mpsc::channel(1);
        let (quit_tx, _rx) = tokio::sync::mpsc::channel(1);
        ThunderTray {
            unread_count,
            badge_color: "#FF0000".into(),
            badge_text_color: "#FFFFFF".into(),
            toggle_tx,
            quit_tx,
        }
    }

    #[test]
    fn test_thunder_tray_title() {
        let t = make_test_tray(0);
        use ksni::Tray;
        assert_eq!(t.title(), "ThunderTray");
    }

    #[test]
    fn test_thunder_tray_title_with_unread() {
        let t = make_test_tray(5);
        use ksni::Tray;
        assert_eq!(t.title(), "ThunderTray - 5 unread");
    }
}
