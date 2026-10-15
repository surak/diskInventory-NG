//! Mounted volume discovery for the start window.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub struct Volume {
    pub name: String,
    pub path: PathBuf,
    pub total: u64,
    pub free: u64,
}

fn capacity(path: &Path) -> Option<(u64, u64)> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let frsize = st.f_frsize;
    Some((st.f_blocks as u64 * frsize, st.f_bavail as u64 * frsize))
}

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
