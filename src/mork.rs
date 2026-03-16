use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Parse a single .msf file and return the unread message count.
/// Returns 0 if the file cannot be parsed or has no unread messages.
pub fn parse_unread_count(msf_path: &Path) -> u32 {
    let count = parse_mork_unread(msf_path);
    if count > 0 {
        return count;
    }
    // Fallback: try parsing companion mbox file
    let mbox_path = msf_path.with_extension(""); // "INBOX.msf" → "INBOX"
    if mbox_path.exists() {
        return parse_mbox_unread(&mbox_path);
    }
    0
}

/// Parse a Mork (.msf) file to extract unread message count.
fn parse_mork_unread(msf_path: &Path) -> u32 {
    let content = match std::fs::read_to_string(msf_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read Mork file {:?}: {}", msf_path, e);
            return 0;
        }
    };

    // Build column dictionary from the dict section: `< ... (hex_id=column_name) ... >`
    // The dict is a `< ... >` block containing `(XX=name)` entries
    let mut columns: HashMap<String, String> = HashMap::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    let mut dict_depth: i32 = 0;

    // Phase 1: parse dictionary entries inside < ... > blocks
    while i < chars.len() {
        if chars[i] == '<' {
            dict_depth += 1;
            i += 1;
            continue;
        }
        if chars[i] == '>' {
            dict_depth -= 1;
            i += 1;
            continue;
        }
        // Only parse column defs at outermost dict level (depth == 1)
        if dict_depth == 1 && chars[i] == '(' {
            i += 1; // Skip '('
            let id_start = i;

            // Extract hex_id until '='
            while i < chars.len() && chars[i] != '=' && chars[i] != ')' {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '=' {
                i += 1;
                continue;
            }

            let hex_id = chars[id_start..i].iter().collect::<String>();
            i += 1; // Skip '='

            // Extract column_name until ')'
            let name_start = i;
            while i < chars.len() && chars[i] != ')' {
                i += 1;
            }
            if i < chars.len() {
                let column_name = chars[name_start..i].iter().collect::<String>();
                columns.insert(hex_id, column_name);
            }
        }
        i += 1;
    }

    // Find target column id for "numNewMsgs" or fallback "numMsgs"
    let mut target_id = None;
    for (id, name) in &columns {
        if name == "numNewMsgs" {
            target_id = Some(id.clone());
            break;
        }
    }
    if target_id.is_none() {
        for (id, name) in &columns {
            if name == "numMsgs" {
                target_id = Some(id.clone());
                break;
            }
        }
    }

    let target_id = match target_id {
        Some(id) => id,
        None => {
            tracing::warn!("No numNewMsgs or numMsgs column found in {:?}", msf_path);
            return 0;
        }
    };

    // Phase 2: scan data rows for (^target_id=value) patterns
    // In mork data, column references use ^ prefix: (^A2=0)
    let mut max_value = 0u32;
    i = 0;

    while i < chars.len() {
        if chars[i] == '(' {
            i += 1;

            // Skip optional ^ prefix
            let has_caret = i < chars.len() && chars[i] == '^';
            if has_caret {
                i += 1;
            }

            let id_start = i;

            while i < chars.len() && chars[i] != '=' && chars[i] != ')' {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '=' {
                continue;
            }

            let field_id = chars[id_start..i].iter().collect::<String>();

            if field_id == target_id {
                i += 1; // Skip '='
                let value_start = i;

                while i < chars.len() && chars[i] != ')' {
                    i += 1;
                }
                if i < chars.len() {
                    let value_str = chars[value_start..i].iter().collect::<String>();

                    let value = if let Ok(v) = value_str.parse::<u32>() {
                        v
                    } else {
                        let hex_str = value_str.trim_start_matches('$');
                        u32::from_str_radix(hex_str, 16).unwrap_or(0)
                    };

                    if value > max_value {
                        max_value = value;
                    }
                }
            }
        }
        i += 1;
    }

    max_value
}

/// Parse an mbox file to count unread messages.
/// Messages are unread if the Read bit (0x0001) is not set and not deleted (0x0008).
pub(crate) fn parse_mbox_unread(mbox_path: &Path) -> u32 {
    let file = match File::open(mbox_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to open mbox file {:?}: {}", mbox_path, e);
            return 0;
        }
    };

    let reader = BufReader::new(file);
    let mut unread_count = 0u32;
    let mut in_headers = false;
    let mut current_status: Option<u16> = None;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        // New message starts with "From "
        if line.starts_with("From ") {
            // Process previous message
            if let Some(status) = current_status {
                let is_read = (status & 0x0001) != 0;
                let is_deleted = (status & 0x0008) != 0;

                if !is_read && !is_deleted {
                    unread_count += 1;
                }
            }

            // Start new message
            in_headers = true;
            current_status = None;
            continue;
        }

        if in_headers {
            // Blank line ends headers
            if line.trim().is_empty() {
                in_headers = false;
                continue;
            }

            // Parse X-Mozilla-Status header
            if line.starts_with("X-Mozilla-Status: ") {
                if let Some(hex_str) = line.strip_prefix("X-Mozilla-Status: ") {
                    if let Ok(status) = u16::from_str_radix(hex_str.trim(), 16) {
                        current_status = Some(status);
                    }
                }
            }
        }
    }

    // Process last message
    if let Some(status) = current_status {
        let is_read = (status & 0x0001) != 0;
        let is_deleted = (status & 0x0008) != 0;

        if !is_read && !is_deleted {
            unread_count += 1;
        }
    }

    unread_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_mbox_unread_count() {
        // Create temp file with 3 messages: 2 unread (status 0000), 1 read (status 0001)
        let mbox_content = "From sender@example.com Mon Jan  1 00:00:00 2024\nX-Mozilla-Status: 0000\nSubject: Unread 1\n\nBody 1\nFrom sender@example.com Mon Jan  1 00:00:01 2024\nX-Mozilla-Status: 0001\nSubject: Read\n\nBody 2\nFrom sender@example.com Mon Jan  1 00:00:02 2024\nX-Mozilla-Status: 0000\nSubject: Unread 2\n\nBody 3\n";
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(mbox_content.as_bytes()).unwrap();
        let count = parse_mbox_unread(tmp.path());
        assert_eq!(count, 2);
    }

    #[test]
    fn test_mbox_with_deleted() {
        // deleted message (0x0008) should not count
        let mbox_content = "From sender@example.com Mon Jan  1 00:00:00 2024\nX-Mozilla-Status: 0008\nSubject: Deleted\n\nBody\n";
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(mbox_content.as_bytes()).unwrap();
        let count = parse_mbox_unread(tmp.path());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(parse_unread_count(tmp.path()), 0);
    }

    #[test]
    fn test_nonexistent_file() {
        assert_eq!(
            parse_unread_count(std::path::Path::new("/nonexistent/file.msf")),
            0
        );
    }
}
