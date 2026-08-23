use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::NamedTempFile;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

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

async fn run_kwin_script(script: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let plugin_name = format!("thundertray_{}_{}", std::process::id(), counter);
    let mut script_file = NamedTempFile::new()?;
    script_file.write_all(script.as_bytes())?;
    script_file.flush()?;
    let tmp_path = script_file.path().to_string_lossy().into_owned();

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

    Ok(())
}

pub async fn show_thunderbird_window() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_kwin_script(SHOW_SCRIPT).await?;
    tracing::debug!("Showed Thunderbird window");
    Ok(())
}

pub async fn toggle_thunderbird_window() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_kwin_script(TOGGLE_SCRIPT).await?;
    tracing::debug!("Toggled Thunderbird window");
    Ok(())
}

pub async fn hide_thunderbird_window() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_kwin_script(HIDE_SCRIPT).await?;
    tracing::debug!("Hid Thunderbird window");
    Ok(())
}

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

    #[test]
    fn test_hide_script_only_targets_thunderbird() {
        assert!(HIDE_SCRIPT.contains("org.mozilla.Thunderbird"));
        assert!(HIDE_SCRIPT.contains("skipTaskbar = true"));
    }
}
