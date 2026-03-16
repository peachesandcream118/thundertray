use ksni::TrayMethods;
use tracing::{info, error};

struct ThunderTray {
    unread_count: u32,
    badge_color: String,
    badge_text_color: String,
    toggle_tx: tokio::sync::mpsc::Sender<()>,
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
        let pixmap = crate::icon::render_icon(
            self.unread_count,
            &self.badge_color,
            &self.badge_text_color,
        );
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
                label: "Settings...".into(),
                activate: Box::new(|_: &mut Self| {
                    crate::settings_gui::open_settings_detached();
                }),
                ..Default::default()
            }),
            ksni::MenuItem::Separator,
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Quit".into(),
                activate: Box::new(|_: &mut Self| {
                    std::process::exit(0);
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
    initial_child: Option<tokio::process::Child>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Initializing ThunderTray system tray");

    let (toggle_tx, mut toggle_rx) = tokio::sync::mpsc::channel::<()>(4);

    let tray = ThunderTray {
        unread_count: 0,
        badge_color: config.appearance.badge_color.clone(),
        badge_text_color: config.appearance.badge_text_color.clone(),
        toggle_tx,
    };

    let handle = tray.spawn().await?;

    info!("Tray service spawned");

    let watcher = crate::watcher::MailWatcher::new(
        msf_files,
        config.monitoring.poll_interval_secs,
    );

    // Spawn toggle handler task with debouncing
    let tb_command = config.general.thunderbird_command.clone();
    let visible = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let vis_clone = visible.clone();
    tokio::spawn(async move {
        let mut last_toggle = std::time::Instant::now() - std::time::Duration::from_secs(1);
        while toggle_rx.recv().await.is_some() {
            // Debounce: ignore clicks within 500ms of last toggle
            if last_toggle.elapsed() < std::time::Duration::from_millis(500) {
                continue;
            }
            last_toggle = std::time::Instant::now();
            let wm = crate::window::WindowManager::new(&tb_command, vis_clone.clone());
            if let Err(e) = wm.toggle_visibility().await {
                error!("Failed to toggle Thunderbird: {}", e);
            }
        }
    });

    // Spawn TB watchdog — event-driven restart when TB exits
    let tb_cmd_wd = config.general.thunderbird_command.clone();
    let vis_wd = visible.clone();
    tokio::spawn(async move {
        // Get initial child handle, or spawn TB now to get one
        let wm = crate::window::WindowManager::new(&tb_cmd_wd, vis_wd.clone());
        let mut child = match initial_child {
            Some(c) => c,
            None => match wm.start_hidden().await {
                Ok(c) => c,
                Err(e) => {
                    error!("Watchdog: could not start Thunderbird: {}", e);
                    return;
                }
            },
        };
        loop {
            // Event-driven: blocks here with zero CPU until TB actually exits
            let status = child.wait().await;
            info!("Thunderbird exited ({:?}) — restarting immediately", status);

            // Brief pause to prevent runaway restart loops
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Restart and get new handle
            let wm = crate::window::WindowManager::new(&tb_cmd_wd, vis_wd.clone());
            loop {
                match wm.start_hidden().await {
                    Ok(new_child) => {
                        child = new_child;
                        break;
                    }
                    Err(e) => {
                        error!("Watchdog: failed to restart Thunderbird: {e}");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // Poll loop — check unread count periodically and update tray
    let mut last_count = 0u32;
    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(config.monitoring.poll_interval_secs),
    );

    loop {
        interval.tick().await;
        let new_count = watcher.get_unread_count();
        if new_count != last_count {
            info!("Unread count changed: {} -> {}", last_count, new_count);
            handle.update(|tray| {
                tray.unread_count = new_count;
            }).await;
            last_count = new_count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_tray(unread_count: u32) -> ThunderTray {
        let (toggle_tx, _rx) = tokio::sync::mpsc::channel(1);
        ThunderTray {
            unread_count,
            badge_color: "#FF0000".into(),
            badge_text_color: "#FFFFFF".into(),
            toggle_tx,
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
