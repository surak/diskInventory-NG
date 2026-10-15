//! Operating-system specific bits: file sizes, the file manager and
//! keyboard shortcut labels.

use std::fs::Metadata;
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

/// Label for "show this item in the file manager".
#[cfg(target_os = "macos")]
pub const REVEAL_LABEL: &str = "Reveal in Finder";
#[cfg(windows)]
pub const REVEAL_LABEL: &str = "Show in Explorer";
#[cfg(not(any(target_os = "macos", windows)))]
pub const REVEAL_LABEL: &str = "Show in File Manager";

/// Prefix for keyboard shortcut hints (egui maps "command" to Ctrl off macOS).
pub const CMD: &str = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };

/// Directories that are never descended into unless they are the scan root:
/// other volumes and virtual filesystems.
#[cfg(target_os = "macos")]
pub const SKIP_DIRS: &[&str] = &["/Volumes", "/System/Volumes", "/dev"];
#[cfg(windows)]
pub const SKIP_DIRS: &[&str] = &[];
#[cfg(not(any(target_os = "macos", windows)))]
pub const SKIP_DIRS: &[&str] = &["/proc", "/sys", "/dev", "/run"];

/// Space the file occupies on disk.
#[cfg(unix)]
pub fn disk_size(md: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    md.blocks() * 512
}
#[cfg(not(unix))]
pub fn disk_size(md: &Metadata) -> u64 {
    md.len()
}

/// Identity of a file with several hard links, so it is counted only once.
#[cfg(unix)]
pub fn hard_link_id(md: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    (md.nlink() > 1).then(|| (md.dev(), md.ino()))
}
#[cfg(not(unix))]
pub fn hard_link_id(_md: &Metadata) -> Option<(u64, u64)> {
    None
}

/// Modification time in seconds since the Unix epoch.
pub fn mtime(md: &Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Shows the item selected in the system file manager.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        // The FileManager1 D-Bus interface highlights the item (Nautilus,
        // Dolphin, Nemo, …); fall back to opening the containing folder.
        let uri = format!("file://{}", path.display());
        let ok = Command::new("dbus-send")
            .args([
                "--session",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
            ])
            .arg(format!("array:string:{uri}"))
            .arg("string:")
            .status()
            .is_ok_and(|s| s.success());
        if let (false, Some(parent)) = (ok, path.parent()) {
            let _ = Command::new("xdg-open").arg(parent).spawn();
        }
    }
}

/// Opens the item with its default application.
pub fn open(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).spawn();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(path).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}
