use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MSG_FLAG_READ: u16 = 0x0001;
const MSG_FLAG_EXPUNGED: u16 = 0x0008;

/// Return the same unread total Thunderbird exposes for a folder.
///
/// Thunderbird persists this value in the profile's `folderCache.json`, keyed
/// by the exact summary-file path. The cache is the authoritative lightweight
/// source: it avoids trying to reconstruct Mork's transaction log and, unlike
/// `numNewMsgs`, represents the real unread total rather than biff-only mail.
pub fn parse_unread_count(msf_path: &Path) -> u32 {
    if let Some(count) = parse_folder_cache_unread(msf_path) {
        return count;
    }

    // An mbox remains a useful degraded fallback if Thunderbird's cache is
    // absent, malformed, or reports an unknown (-1) server count.
    let mbox_path = msf_path.with_extension(""); // "INBOX.msf" → "INBOX"
    if mbox_path.exists() {
        return parse_mbox_unread(&mbox_path);
    }

    0
}

fn parse_folder_cache_unread(msf_path: &Path) -> Option<u32> {
    let cache_path = find_folder_cache(msf_path)?;
    let contents = std::fs::read(cache_path).ok()?;
    let cache: serde_json::Value = serde_json::from_slice(&contents).ok()?;
    let entries = cache.as_object()?;
    let entry = folder_cache_entry(entries, msf_path)?;

    let total = json_i64(entry.get("totalUnreadMsgs")?)?;
    // `-1` means Thunderbird has no known server count, so let the mbox
    // fallback provide an answer rather than displaying a made-up zero.
    if total < 0 {
        return None;
    }

    // Thunderbird's `getNumUnread` includes pending messages as well as the
    // stored folder total. Negative pending values are not meaningful here.
    let pending = entry
        .get("pendingUnreadMsgs")
        .and_then(json_i64)
        .unwrap_or(0)
        .max(0);

    Some(
        (total as u64)
            .saturating_add(pending as u64)
            .min(u32::MAX as u64) as u32,
    )
}

fn find_folder_cache(msf_path: &Path) -> Option<PathBuf> {
    msf_path.parent()?.ancestors().find_map(|directory| {
        let candidate = directory.join("folderCache.json");
        candidate.is_file().then_some(candidate)
    })
}

fn folder_cache_entry<'a>(
    entries: &'a serde_json::Map<String, serde_json::Value>,
    msf_path: &Path,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    let mut keys = vec![msf_path.to_string_lossy().into_owned()];
    if let Ok(canonical) = msf_path.canonicalize() {
        let canonical = canonical.to_string_lossy().into_owned();
        if !keys.contains(&canonical) {
            keys.push(canonical);
        }
    }

    keys.into_iter()
        .find_map(|key| entries.get(&key)?.as_object())
}

fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

/// Parse an mbox file to count unread messages.
/// Messages are unread when the Read bit is unset and the Expunged bit is not
/// set. Thunderbird writes both flags to `X-Mozilla-Status`.
pub(crate) fn parse_mbox_unread(mbox_path: &Path) -> u32 {
    let file = match File::open(mbox_path) {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!("Failed to open mbox file {mbox_path:?}: {error}");
            return 0;
        }
    };

    let reader = BufReader::new(file);
    let mut unread_count = 0u32;
    let mut in_headers = false;
    let mut current_status = None;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(error) => {
                tracing::warn!("Failed to read mbox line from {mbox_path:?}: {error}");
                continue;
            }
        };

        if line.starts_with("From ") {
            unread_count = unread_count.saturating_add(unread_from_status(current_status));
            in_headers = true;
            current_status = None;
            continue;
        }

        if !in_headers {
            continue;
        }
        if line.trim().is_empty() {
            in_headers = false;
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("X-Mozilla-Status") {
                current_status = u16::from_str_radix(value.trim(), 16).ok();
            }
        }
    }

    unread_count.saturating_add(unread_from_status(current_status))
}

fn unread_from_status(status: Option<u16>) -> u32 {
    status.is_some_and(|status| status & (MSG_FLAG_READ | MSG_FLAG_EXPUNGED) == 0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn summary_path(profile: &Path) -> PathBuf {
        let summary_path = profile.join("ImapMail/example.invalid/INBOX.msf");
        std::fs::create_dir_all(summary_path.parent().unwrap()).unwrap();
        std::fs::write(&summary_path, "summary is owned by Thunderbird").unwrap();
        summary_path
    }

    fn write_folder_cache(profile: &Path, summary: &Path, entry: serde_json::Value) {
        let mut entries = serde_json::Map::new();
        entries.insert(summary.to_string_lossy().into_owned(), entry);
        std::fs::write(
            profile.join("folderCache.json"),
            serde_json::to_vec(&serde_json::Value::Object(entries)).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn exact_folder_cache_entry_is_authoritative_and_includes_pending() {
        let profile = tempfile::tempdir().unwrap();
        let summary = summary_path(profile.path());
        write_folder_cache(
            profile.path(),
            &summary,
            serde_json::json!({
                "totalUnreadMsgs": 540,
                "pendingUnreadMsgs": 13,
                "totalMsgs": 999,
                "serverUnseen": 553,
            }),
        );

        assert_eq!(parse_unread_count(&summary), 553);
    }

    #[test]
    fn cache_lookup_uses_the_full_summary_path_not_just_inbox_name() {
        let profile = tempfile::tempdir().unwrap();
        let summary = summary_path(profile.path());
        let other_summary = profile.path().join("ImapMail/other.invalid/INBOX.msf");
        let mbox_path = summary.with_extension("");
        std::fs::write(
            &mbox_path,
            "From sender@example.com Mon Jan  1 00:00:00 2024\nX-Mozilla-Status: 0000\n\nBody\n",
        )
        .unwrap();

        let mut entries = serde_json::Map::new();
        entries.insert(
            other_summary.to_string_lossy().into_owned(),
            serde_json::json!({"totalUnreadMsgs": 99, "pendingUnreadMsgs": 0}),
        );
        std::fs::write(
            profile.path().join("folderCache.json"),
            serde_json::to_vec(&serde_json::Value::Object(entries)).unwrap(),
        )
        .unwrap();

        assert_eq!(parse_unread_count(&summary), 1);
    }

    #[test]
    fn unknown_cache_total_falls_back_to_mbox_status() {
        let profile = tempfile::tempdir().unwrap();
        let summary = summary_path(profile.path());
        let mbox_path = summary.with_extension("");
        write_folder_cache(
            profile.path(),
            &summary,
            serde_json::json!({"totalUnreadMsgs": -1, "pendingUnreadMsgs": 0}),
        );
        std::fs::write(
            &mbox_path,
            "From sender@example.com Mon Jan  1 00:00:00 2024\nX-Mozilla-Status: 0000\n\nBody\n",
        )
        .unwrap();

        assert_eq!(parse_unread_count(&summary), 1);
    }

    #[test]
    fn mbox_unread_count_excludes_read_and_expunged_messages() {
        let mbox_content = "From sender@example.com Mon Jan  1 00:00:00 2024\nX-Mozilla-Status: 0000\n\nUnread\nFrom sender@example.com Mon Jan  1 00:00:01 2024\nx-mozilla-status: 0001\n\nRead\nFrom sender@example.com Mon Jan  1 00:00:02 2024\nX-Mozilla-Status: 0008\n\nExpunged\n";
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(mbox_content.as_bytes()).unwrap();

        assert_eq!(parse_mbox_unread(tmp.path()), 1);
    }

    #[test]
    fn empty_or_missing_sources_report_zero() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(parse_unread_count(tmp.path()), 0);
        assert_eq!(parse_unread_count(Path::new("/nonexistent/INBOX.msf")), 0);
    }
}
