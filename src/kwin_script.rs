use std::sync::atomic::{AtomicU64, Ordering};
use zbus::Connection;
use zbus::zvariant::ObjectPath;

static INVOCATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Show Thunderbird: restore to taskbar, un-minimize, and focus
const SHOW_SCRIPT: &str = r#"
var clients = workspace.windowList();
for (var i = 0; i < clients.length; i++) {
    var c = clients[i];
    if (c.resourceClass === "org.mozilla.Thunderbird" || c.resourceName === "thunderbird") {
        c.skipTaskbar = false;
        c.skipSwitcher = false;
        c.minimized = false;
        workspace.activeWindow = c;
        break;
    }
}
"#;

async fn run_kwin_script(script: &str) -> Result<(), Box<dyn std::error::Error>> {
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let plugin_name = format!("thundertray_{}", counter);
    let tmp_path = format!("/tmp/{}.js", plugin_name);

    std::fs::write(&tmp_path, script.as_bytes())?;

    let connection = Connection::session().await?;

    let reply: i32 = connection
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(tmp_path.as_str(), plugin_name.as_str()),
        )
        .await?
        .body()
        .deserialize()?;

    tracing::debug!("KWin loadScript returned id={} for {}", reply, plugin_name);

    if reply < 0 {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("KWin loadScript failed (returned {})", reply).into());
    }

    let script_path_str = format!("/Scripting/Script{}", reply);
    let script_obj_path = ObjectPath::try_from(script_path_str.as_str())?;

    connection
        .call_method(
            Some("org.kde.KWin"),
            &script_obj_path,
            Some("org.kde.kwin.Script"),
            "run",
            &(),
        )
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let _ = connection
        .call_method(
            Some("org.kde.KWin"),
            &script_obj_path,
            Some("org.kde.kwin.Script"),
            "stop",
            &(),
        )
        .await;

    let _ = connection
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "unloadScript",
            &(plugin_name.as_str(),),
        )
        .await;

    let _ = std::fs::remove_file(&tmp_path);
    Ok(())
}

pub async fn show_thunderbird_window() -> Result<(), Box<dyn std::error::Error>> {
    run_kwin_script(SHOW_SCRIPT).await?;
    tracing::debug!("Showed Thunderbird window");
    Ok(())
}

pub async fn toggle_thunderbird_window() -> Result<(), Box<dyn std::error::Error>> {
    run_kwin_script(TOGGLE_SCRIPT).await?;
    tracing::debug!("Toggled Thunderbird window");
    Ok(())
}

/// KWin script that toggles TB visibility based on actual window state (not Rust-side tracking).
/// Checks skipTaskbar to determine current state — always correct regardless of external changes.
const TOGGLE_SCRIPT: &str = r#"
var clients = workspace.windowList();
for (var i = 0; i < clients.length; i++) {
    var c = clients[i];
    if (c.resourceClass === "org.mozilla.Thunderbird" || c.resourceName === "thunderbird") {
        if (c.skipTaskbar) {
            c.skipTaskbar = false;
            c.skipSwitcher = false;
            c.minimized = false;
            workspace.activeWindow = c;
        } else {
            c.minimized = true;
            c.skipTaskbar = true;
            c.skipSwitcher = true;
        }
        break;
    }
}
"#;

/// Persistent KWin listener that:
/// 1. Auto-hides any new Thunderbird window the instant it appears
/// 2. Watches for external activation (e.g. notification click) and restores the window
const AUTO_HIDE_LISTENER: &str = r#"
function connectActivation(client) {
    var addTime = Date.now();
    client.activeChanged.connect(function() {
        if (!client.active) return;
        // Ignore activation within 2s of window creation (auto-hide takes priority)
        if ((Date.now() - addTime) < 2000) return;
        // External activation (notification click etc) — restore the window
        if (client.skipTaskbar) {
            client.skipTaskbar = false;
            client.skipSwitcher = false;
            client.minimized = false;
        }
    });
}

// Handle new TB windows
workspace.windowAdded.connect(function(client) {
    if (client.resourceClass === "org.mozilla.Thunderbird" || client.resourceName === "thunderbird") {
        client.minimized = true;
        client.skipTaskbar = true;
        client.skipSwitcher = true;
        connectActivation(client);
    }
});

// Handle existing TB windows (already running when ThunderTray starts)
var existingClients = workspace.windowList();
for (var i = 0; i < existingClients.length; i++) {
    (function(c) {
        if (c.resourceClass === "org.mozilla.Thunderbird" || c.resourceName === "thunderbird") {
            connectActivation(c);
        }
    })(existingClients[i]);
}
"#;

/// Install the persistent auto-hide listener. Call once at startup.
/// Returns a handle that can be used to uninstall on shutdown (optional — KWin cleans up on disconnect).
pub async fn install_persistent_auto_hide() -> Result<i32, Box<dyn std::error::Error>> {
    let plugin_name = format!("thundertray_autohide_{}", std::process::id());
    let tmp_path = format!("/tmp/{}.js", plugin_name);

    std::fs::write(&tmp_path, AUTO_HIDE_LISTENER.as_bytes())?;

    let connection = Connection::session().await?;

    // Unload any stale script with similar names from previous runs
    for suffix in [std::process::id().to_string(), "0".to_string(), "1".to_string()] {
        let old_name = format!("thundertray_autohide_{}", suffix);
        let _ = connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "unloadScript",
                &(old_name.as_str(),),
            )
            .await;
    }

    let script_id: i32 = connection
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(tmp_path.as_str(), plugin_name.as_str()),
        )
        .await?
        .body()
        .deserialize()?;

    if script_id < 0 {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("KWin loadScript for auto-hide failed (returned {})", script_id).into());
    }

    let script_path_str = format!("/Scripting/Script{}", script_id);
    let script_obj_path = ObjectPath::try_from(script_path_str.as_str())?;

    connection
        .call_method(
            Some("org.kde.KWin"),
            &script_obj_path,
            Some("org.kde.kwin.Script"),
            "run",
            &(),
        )
        .await?;

    tracing::info!("Persistent auto-hide KWin listener installed: {}", plugin_name);
    // Don't delete the tmp file — KWin needs it while the script is loaded
    Ok(script_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_script_content() {
        assert!(SHOW_SCRIPT.contains("org.mozilla.Thunderbird"));
        assert!(SHOW_SCRIPT.contains("activeWindow"));
    }

    #[test]
    fn test_toggle_script_content() {
        assert!(TOGGLE_SCRIPT.contains("org.mozilla.Thunderbird"));
        assert!(TOGGLE_SCRIPT.contains("skipTaskbar"));
    }
}
