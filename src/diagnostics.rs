//! Small, bounded structured diagnostic log primitives shared by WLOC and the
//! interception path.  The log is intentionally a recent-history buffer: it
//! is not an audit trail and must remain safe on small gateway storage.

use std::path::Path;

pub(crate) const MAX_EVENT_LOG_BYTES: usize = 64 * 1024;
pub(crate) const MAX_EVENT_LINE_BYTES: usize = 2048;

/// Append one JSON event while retaining only complete, newest records.
pub(crate) fn append_json_line(path: &Path, value: &serde_json::Value) {
    let mut line = serde_json::to_string(value).unwrap_or_default();
    line.push('\n');
    if line.len() > MAX_EVENT_LINE_BYTES {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let existing = std::fs::read(path).unwrap_or_default();
    if existing.len().saturating_add(line.len()) <= MAX_EVENT_LOG_BYTES {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write as _;
            let _ = file.write_all(line.as_bytes());
        }
        return;
    }

    let keep_bytes = MAX_EVENT_LOG_BYTES.saturating_sub(line.len());
    let mut retained = if existing.len() > keep_bytes {
        let start = existing.len() - keep_bytes;
        let boundary = existing[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset + 1)
            .unwrap_or(existing.len());
        existing[boundary..].to_vec()
    } else {
        existing
    };
    retained.extend_from_slice(line.as_bytes());
    let temporary = path.with_extension(format!("log.tmp.{}", std::process::id()));
    if std::fs::write(&temporary, retained).is_ok() {
        let _ = std::fs::rename(temporary, path);
    }
}
