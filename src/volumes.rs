//! Mounted volume discovery for the start window.

use std::path::PathBuf;

pub struct Volume {
    pub name: String,
    pub path: PathBuf,
    pub total: u64,
    pub free: u64,
}

#[cfg(target_os = "macos")]
fn capacity(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let frsize = st.f_frsize;
    Some((st.f_blocks as u64 * frsize, st.f_bavail as u64 * frsize))
}

#[cfg(target_os = "macos")]
pub fn list() -> Vec<Volume> {
    let mut paths = vec![PathBuf::from("/")];
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        let mut more: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            // The boot volume shows up in /Volumes as a symlink to "/".
            .filter(|p| !p.is_symlink())
            .collect();
        more.sort();
        paths.extend(more);
    }
    paths
        .into_iter()
        .filter_map(|path| {
            let (total, free) = capacity(&path)?;
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => "Startup Disk (/)".to_owned(),
            };
            Some(Volume { name, path, total, free })
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn list() -> Vec<Volume> {
    use sysinfo::Disks;

    // Pseudo and read-only image filesystems are not worth scanning.
    const IGNORED_FS: &[&str] = &["squashfs", "overlay", "tmpfs", "devtmpfs", "efivarfs", "ramfs"];

    let disks = Disks::new_with_refreshed_list();
    let mut seen = std::collections::HashSet::new();
    let mut volumes: Vec<Volume> = disks
        .list()
        .iter()
        .filter(|d| {
            let fs = d.file_system().to_string_lossy();
            d.total_space() > 0 && !IGNORED_FS.contains(&fs.as_ref())
        })
        .filter(|d| seen.insert(d.mount_point().to_path_buf()))
        .map(|d| {
            let path = d.mount_point().to_path_buf();
            let label = d.name().to_string_lossy().into_owned();
            let name = match (label.is_empty(), path.to_string_lossy().into_owned()) {
                (true, p) if p == "/" => "Root (/)".to_owned(),
                (true, p) => p,
                (false, p) => format!("{label} ({})", p.trim_end_matches(['\\', '/'])),
            };
            Volume { name, path, total: d.total_space(), free: d.available_space() }
        })
        .collect();
    volumes.sort_by(|a, b| a.path.cmp(&b.path));
    volumes
}
