//! Audit logging for sensitive operations.
//!
//! Writes structured entries to `<data_dir>/audit.log` so that
//! secret changes, pipeline runs, and AI interactions are traceable.

use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

/// Log an audit event. Failures are logged via `log::warn` but never
/// propagated — audit logging must not block the operation itself.
pub fn log_event(action: &str, details: &str) {
    let timestamp = Utc::now().to_rfc3339();
    let line = format!("{} | {} | {}\n", timestamp, action, details);

    if let Err(e) = write_audit_line(&line) {
        log::warn!("Failed to write audit log: {}", e);
    }
}

fn write_audit_line(line: &str) -> anyhow::Result<()> {
    let dir = super::persistence::data_dir()?;
    let audit_file = dir.join("audit.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_file)?;

    // Ensure the audit file is owner-only (on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&audit_file, perms);
    }

    file.write_all(line.as_bytes())?;
    Ok(())
}
