//! Filesystem scanning: builds an arena-allocated tree of files and folders
//! together with per-extension ("file kind") statistics.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;

use crate::platform;

/// Kind index used for directories.
pub const NO_KIND: u32 = u32::MAX;

pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    /// Children, sorted by size (largest first) once the scan is finished.
    pub children: Vec<usize>,
    /// Allocated size on disk in bytes (recursive for directories).
    pub size: u64,
    pub is_dir: bool,
    pub kind: u32,
    /// Number of files contained (1 for a file).
    pub files: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
}

pub struct KindStat {
    pub name: String,
    pub size: u64,
    pub count: u64,
    pub color: [u8; 3],
}

pub struct Tree {
    /// `nodes[0]` is the root; its `name` holds the full scanned path.
    pub nodes: Vec<Node>,
    /// Sorted by total size, largest first.
    pub kinds: Vec<KindStat>,
}

impl Tree {
    pub fn path(&self, mut idx: usize) -> PathBuf {
        let mut parts = Vec::new();
        loop {
            let n = &self.nodes[idx];
            parts.push(n.name.as_str());
            match n.parent {
                Some(p) => idx = p,
                None => break,
            }
        }
        parts.iter().rev().collect()
    }

    pub fn is_ancestor(&self, ancestor: usize, mut idx: usize) -> bool {
        loop {
            if idx == ancestor {
                return true;
            }
            match self.nodes[idx].parent {
                Some(p) => idx = p,
                None => return false,
            }
        }
    }

    pub fn kind_name(&self, idx: usize) -> &str {
        match self.nodes[idx].kind {
            NO_KIND => "Folder",
            k => &self.kinds[k as usize].name,
        }
    }

    /// Detaches a node (after it was deleted on disk) and fixes up all sizes
    /// and kind statistics.
    pub fn remove(&mut self, idx: usize) {
        let Some(parent) = self.nodes[idx].parent else { return };
        let (size, files) = (self.nodes[idx].size, self.nodes[idx].files);

        let mut stack = vec![idx];
        while let Some(i) = stack.pop() {
            let n = &self.nodes[i];
            if n.kind != NO_KIND {
                let k = &mut self.kinds[n.kind as usize];
                k.size -= n.size;
                k.count -= 1;
            }
            stack.extend_from_slice(&n.children);
        }

        self.nodes[parent].children.retain(|&c| c != idx);
        let mut p = Some(parent);
        while let Some(i) = p {
            self.nodes[i].size -= size;
            self.nodes[i].files -= files;
            p = self.nodes[i].parent;
        }
    }
}

impl Tree {
    /// Replaces the contents of directory `idx` with a fresh scan of it
    /// (`sub`, whose root is that directory), fixing up the sizes of all
    /// ancestors and the kind statistics. The old nodes stay in the arena,
    /// unreferenced.
    pub fn replace_subtree(&mut self, idx: usize, sub: Tree) {
        // Forget the old contents' kind statistics.
        let mut stack = self.nodes[idx].children.clone();
        while let Some(i) = stack.pop() {
            let n = &self.nodes[i];
            if n.kind != NO_KIND {
                let k = &mut self.kinds[n.kind as usize];
                k.size -= n.size;
                k.count -= 1;
            }
            stack.extend_from_slice(&n.children);
        }

        // Map the new scan's kinds onto ours, adding unknown ones.
        let mut by_name: HashMap<String, u32> =
            self.kinds.iter().enumerate().map(|(i, k)| (k.name.clone(), i as u32)).collect();
        let remap: Vec<u32> = sub
            .kinds
            .iter()
            .map(|k| {
                *by_name.entry(k.name.clone()).or_insert_with(|| {
                    let color = crate::kinds::color_for_rank(self.kinds.len());
                    self.kinds.push(KindStat { name: k.name.clone(), size: 0, count: 0, color });
                    self.kinds.len() as u32 - 1
                })
            })
            .collect();
        for (k, stat) in sub.kinds.iter().enumerate() {
            let ours = &mut self.kinds[remap[k] as usize];
            ours.size += stat.size;
            ours.count += stat.count;
        }

        // Append the new nodes; sub index j (j ≥ 1) becomes offset + j.
        let (old_size, old_files) = (self.nodes[idx].size, self.nodes[idx].files);
        let offset = self.nodes.len() - 1;
        let mut nodes = sub.nodes.into_iter();
        let root = nodes.next().expect("scan always has a root");
        for mut n in nodes {
            n.parent = n.parent.map(|p| if p == 0 { idx } else { p + offset });
            for c in &mut n.children {
                *c += offset;
            }
            if n.kind != NO_KIND {
                n.kind = remap[n.kind as usize];
            }
            self.nodes.push(n);
        }
        let node = &mut self.nodes[idx];
        node.children = root.children.iter().map(|&c| c + offset).collect();
        node.size = root.size;
        node.files = root.files;
        node.mtime = root.mtime;

        // Ancestors: adjust totals and keep children sorted by size.
        let mut p = self.nodes[idx].parent;
        while let Some(i) = p {
            let n = &mut self.nodes[i];
            n.size = n.size + root.size - old_size;
            n.files = n.files + root.files - old_files;
            let mut children = std::mem::take(&mut n.children);
            children.sort_by(|&a, &b| self.nodes[b].size.cmp(&self.nodes[a].size));
            self.nodes[i].children = children;
            p = self.nodes[i].parent;
        }
    }
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub current: Mutex<String>,
    pub cancel: AtomicBool,
}


/// Result of scanning one directory entry, before it is flattened into the
/// arena. Built in parallel, one task per directory.
enum Entry {
    File { name: String, size: u64, mtime: i64 },
    Dir { name: String, mtime: i64, children: Vec<Entry> },
}

struct Walker<'a> {
    seen_inodes: Mutex<HashSet<(u64, u64)>>,
    progress: &'a Progress,
}

impl Walker<'_> {
    fn walk(&self, path: &Path) -> Vec<Entry> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }
        // Only a progress hint; never wait for other threads.
        if let Ok(mut cur) = self.progress.current.try_lock() {
            *cur = path.to_string_lossy().into_owned();
        }
        let Ok(entries) = fs::read_dir(path) else { return Vec::new() };

        let mut files = Vec::new();
        let mut dirs = Vec::new();
        let (mut count, mut bytes) = (0, 0);
        for entry in entries.flatten() {
            // DirEntry::metadata does not follow symlinks.
            let Ok(md) = entry.metadata() else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            if md.is_dir() {
                let child_path = entry.path();
                if !platform::SKIP_DIRS.iter().any(|d| child_path == Path::new(d)) {
                    dirs.push((name, platform::mtime(&md), child_path));
                }
            } else {
                // Count hard-linked files only once.
                let size = match platform::hard_link_id(&md) {
                    Some(id) if !self.seen_inodes.lock().unwrap().insert(id) => 0,
                    _ => platform::disk_size(&md),
                };
                count += 1;
                bytes += size;
                files.push(Entry::File { name, size, mtime: platform::mtime(&md) });
            }
        }
        self.progress.files.fetch_add(count, Ordering::Relaxed);
        self.progress.bytes.fetch_add(bytes, Ordering::Relaxed);

        let subdirs: Vec<Entry> = dirs
            .into_par_iter()
            .map(|(name, mtime, p)| Entry::Dir { name, mtime, children: self.walk(&p) })
            .collect();
        files.extend(subdirs);
        files
    }
}

struct Scanner {
    nodes: Vec<Node>,
    kind_map: HashMap<String, u32>,
    kinds: Vec<KindStat>,
}

/// Scans `root` recursively, using all CPU cores. Returns `None` if cancelled.
pub fn scan(root: &Path, progress: &Progress) -> Option<Tree> {
    let walker = Walker { seen_inodes: Mutex::new(HashSet::new()), progress };
    let entries = walker.walk(root);
    if progress.cancel.load(Ordering::Relaxed) {
        return None;
    }

    let mut s = Scanner { nodes: Vec::new(), kind_map: HashMap::new(), kinds: Vec::new() };
    let mtime = fs::symlink_metadata(root).map(|m| platform::mtime(&m)).unwrap_or(0);
    s.nodes.push(Node {
        name: root.to_string_lossy().into_owned(),
        parent: None,
        children: Vec::new(),
        size: 0,
        is_dir: true,
        kind: NO_KIND,
        files: 0,
        mtime,
    });
    s.add_children(0, entries);
    Some(s.finish())
}

impl Scanner {
    fn kind_for(&mut self, name: &str) -> u32 {
        let key = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 12 => {
                format!(".{}", ext.to_lowercase())
            }
            _ => "(no extension)".to_owned(),
        };
        if let Some(&k) = self.kind_map.get(&key) {
            return k;
        }
        let k = self.kinds.len() as u32;
        self.kinds.push(KindStat { name: key.clone(), size: 0, count: 0, color: [0; 3] });
        self.kind_map.insert(key, k);
        k
    }

    /// Flattens scanned entries into the arena below `idx`.
    fn add_children(&mut self, idx: usize, entries: Vec<Entry>) {
        for entry in entries {
            let child = self.nodes.len();
            match entry {
                Entry::File { name, size, mtime } => {
                    let kind = self.kind_for(&name);
                    let k = &mut self.kinds[kind as usize];
                    k.size += size;
                    k.count += 1;
                    self.nodes.push(Node {
                        name,
                        parent: Some(idx),
                        children: Vec::new(),
                        size,
                        is_dir: false,
                        kind,
                        files: 1,
                        mtime,
                    });
                }
                Entry::Dir { name, mtime, children } => {
                    self.nodes.push(Node {
                        name,
                        parent: Some(idx),
                        children: Vec::new(),
                        size: 0,
                        is_dir: true,
                        kind: NO_KIND,
                        files: 0,
                        mtime,
                    });
                    self.add_children(child, children);
                }
            }
            let (size, files) = (self.nodes[child].size, self.nodes[child].files);
            let parent = &mut self.nodes[idx];
            parent.children.push(child);
            parent.size += size;
            parent.files += files;
        }
    }

    fn finish(mut self) -> Tree {
        // Sort children largest first (required by the treemap layout).
        for i in 0..self.nodes.len() {
            let mut children = std::mem::take(&mut self.nodes[i].children);
            children.sort_by(|&a, &b| self.nodes[b].size.cmp(&self.nodes[a].size));
            self.nodes[i].children = children;
        }

        // Order kinds by size and remap the indices stored in the nodes.
        let mut order: Vec<usize> = (0..self.kinds.len()).collect();
        order.sort_by(|&a, &b| self.kinds[b].size.cmp(&self.kinds[a].size));
        let mut remap = vec![0u32; order.len()];
        for (new, &old) in order.iter().enumerate() {
            remap[old] = new as u32;
        }
        for n in &mut self.nodes {
            if n.kind != NO_KIND {
                n.kind = remap[n.kind as usize];
            }
        }
        let mut old: Vec<Option<KindStat>> = self.kinds.into_iter().map(Some).collect();
        let mut kinds: Vec<KindStat> = order.iter().map(|&o| old[o].take().unwrap()).collect();
        for (rank, k) in kinds.iter_mut().enumerate() {
            k.color = crate::kinds::color_for_rank(rank);
        }

        Tree { nodes: self.nodes, kinds }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    fn find(tree: &Tree, name: &str) -> usize {
        tree.nodes.iter().position(|n| n.name == name).unwrap()
    }

    #[test]
    fn rescanning_a_subtree_matches_a_full_scan() {
        let root = std::env::temp_dir().join(format!("dinv-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/deep")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        write(&root.join("a/one.txt"), 10_000);
        write(&root.join("a/deep/two.bin"), 50_000);
        write(&root.join("b/three.txt"), 20_000);

        let mut tree = scan(&root, &Progress::default()).unwrap();

        // Clean up and add something inside "a", then rescan only "a".
        fs::remove_file(root.join("a/deep/two.bin")).unwrap();
        write(&root.join("a/new.jpg"), 300_000);
        let a = find(&tree, "a");
        let sub = scan(&root.join("a"), &Progress::default()).unwrap();
        tree.replace_subtree(a, sub);

        let full = scan(&root, &Progress::default()).unwrap();
        assert_eq!(tree.nodes[0].size, full.nodes[0].size);
        assert_eq!(tree.nodes[0].files, full.nodes[0].files);
        assert_eq!(tree.nodes[a].size, full.nodes[find(&full, "a")].size);
        // The biggest child comes first again.
        assert_eq!(tree.nodes[0].children[0], a);
        for name in [".txt", ".jpg", ".bin"] {
            let count = |t: &Tree| t.kinds.iter().find(|k| k.name == name).map_or(0, |k| k.count);
            assert_eq!(count(&tree), count(&full), "{name}");
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
