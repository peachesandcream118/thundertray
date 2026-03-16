use std::sync::atomic::{AtomicU64, Ordering};
use zbus::Connection;
use zbus::zvariant::ObjectPath;

static INVOCATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Hide Thunderbird: minimize + remove from taskbar and alt-tab
const HIDE_SCRIPT: &str = r#"
var clients = workspace.windowList();
for (var i = 0; i < clients.length; i++) {
    var c = clients[i];
    if (c.resourceClass === "org.mozilla.Thunderbird" || c.resourceName === "thunderbird") {
        c.minimized = true;
        c.skipTaskbar = true;
        c.skipSwitcher = true;
        break;
    }
}
"#;

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

pub async fn hide_thunderbird_window() -> Result<(), Box<dyn std::error::Error>> {
    run_kwin_script(HIDE_SCRIPT).await?;
    tracing::debug!("Hid Thunderbird window");
    Ok(())
}

/// Persistent KWin listener that auto-hides any new Thunderbird window the instant it appears.
/// This script stays loaded for the lifetime of thundertray — `windowAdded` only fires for
/// NEW windows, so it won't interfere with show/hide toggle on existing windows.
const AUTO_HIDE_LISTENER: &str = r#"
workspace.windowAdded.connect(function(client) {
    if (client.resourceClass === "org.mozilla.Thunderbird" || client.resourceName === "thunderbird") {
        client.minimized = true;
        client.skipTaskbar = true;
        client.skipSwitcher = true;
    }
});
"#;

/// Install the persistent auto-hide listener. Call once at startup.
/// Returns a handle that can be used to uninstall on shutdown (optional — KWin cleans up on disconnect).
pub async fn install_persistent_auto_hide() -> Result<i32, Box<dyn std::error::Error>> {
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let plugin_name = format!("thundertray_autohide_{}", counter);
    let tmp_path = format!("/tmp/{}.js", plugin_name);

    std::fs::write(&tmp_path, AUTO_HIDE_LISTENER.as_bytes())?;

    let connection = Connection::session().await?;

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
    fn test_hide_script_content() {
        assert!(HIDE_SCRIPT.contains("org.mozilla.Thunderbird"));
        assert!(HIDE_SCRIPT.contains("skipTaskbar"));
        assert!(HIDE_SCRIPT.contains("skipSwitcher"));
    }

    #[test]
    fn test_show_script_content() {
        assert!(SHOW_SCRIPT.contains("org.mozilla.Thunderbird"));
        assert!(SHOW_SCRIPT.contains("activeWindow"));
    }
}
