//! Small macOS integrations.

use std::path::Path;
use std::process::Command;

/// Ask the user to pick a CSV file (native macOS file dialog via AppleScript).
pub fn choose_csv_file() -> Option<std::path::PathBuf> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(r#"set f to choose file with prompt "Import tasks from a CSV file" of type {"public.comma-separated-values-text", "public.plain-text", "csv"}"#)
        .arg("-e")
        .arg("POSIX path of f")
        .output()
        .ok()?;
    if !out.status.success() {
        return None; // cancelled
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then(|| path.into())
}

pub fn downloads_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Downloads")
}

/// Where the daily CSV backups go.
pub fn documents_backup_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Documents/emTasks")
}

pub fn reveal_in_finder(path: &Path) {
    let _ = Command::new("open").arg("-R").arg(path).spawn();
}
