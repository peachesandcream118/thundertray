use std::collections::HashSet;
use std::path::PathBuf;

pub struct MailWatcher {
    msf_files: Vec<PathBuf>,
}

impl MailWatcher {
    pub fn new(msf_files: Vec<PathBuf>, _poll_interval_secs: u64) -> Self {
        let mut seen = HashSet::new();
        let msf_files = msf_files
            .into_iter()
            .filter(|path| {
                // Profile discovery can encounter the same file through an
                // alias or symlink. Count a mailbox once, not once per path.
                let identity = path.canonicalize().unwrap_or_else(|_| path.clone());
                seen.insert(identity)
            })
            .collect();

        Self { msf_files }
    }

    /// Get total unread count across all monitored .msf files right now
    pub fn get_unread_count(&self) -> u32 {
        saturating_total(
            self.msf_files
                .iter()
                .map(|path| crate::mork::parse_unread_count(path)),
        )
    }
}

fn saturating_total(counts: impl IntoIterator<Item = u32>) -> u32 {
    counts
        .into_iter()
        .fold(0u32, |total, count| total.saturating_add(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_watcher() {
        let w = MailWatcher::new(vec![], 5);
        assert_eq!(w.get_unread_count(), 0);
    }

    #[test]
    fn test_nonexistent_files() {
        let w = MailWatcher::new(vec![PathBuf::from("/nonexistent/INBOX.msf")], 5);
        assert_eq!(w.get_unread_count(), 0);
    }

    #[test]
    fn duplicate_paths_are_counted_once() {
        let profile = tempfile::tempdir().unwrap();
        let msf = profile.path().join("ImapMail/example.invalid/INBOX.msf");
        std::fs::create_dir_all(msf.parent().unwrap()).unwrap();
        std::fs::write(&msf, "summary is owned by Thunderbird").unwrap();
        let mut entries = serde_json::Map::new();
        entries.insert(
            msf.to_string_lossy().into_owned(),
            serde_json::json!({"totalUnreadMsgs": 1, "pendingUnreadMsgs": 0}),
        );
        std::fs::write(
            profile.path().join("folderCache.json"),
            serde_json::to_vec(&serde_json::Value::Object(entries)).unwrap(),
        )
        .unwrap();

        let watcher = MailWatcher::new(vec![msf.clone(), msf], 5);
        assert_eq!(watcher.get_unread_count(), 1);
    }

    #[test]
    fn totals_saturate_instead_of_wrapping() {
        assert_eq!(saturating_total([u32::MAX, 1]), u32::MAX);
    }
}
