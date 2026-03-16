use std::path::PathBuf;

pub struct MailWatcher {
    msf_files: Vec<PathBuf>,
}

impl MailWatcher {
    pub fn new(msf_files: Vec<PathBuf>, _poll_interval_secs: u64) -> Self {
        Self {
            msf_files,
        }
    }

    /// Get total unread count across all monitored .msf files right now
    pub fn get_unread_count(&self) -> u32 {
        self.msf_files
            .iter()
            .map(|path| crate::mork::parse_unread_count(path))
            .sum()
    }
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
}
