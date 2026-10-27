use std::ffi::OsString;
use std::path::PathBuf;

pub type NodeId = u32;
pub const NONE: NodeId = u32::MAX;

#[derive(Debug)]
pub struct Node {
    pub name: OsString,
    pub bytes: u64,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub next_sibling: NodeId,
    pub is_dir: bool,
}

#[derive(Debug, Default)]
pub struct Snapshot {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    pub files: u64,
    pub folders: u64,
    pub errors: u64,
    pub skipped: u64,
    pub issues: Vec<String>,
    pub types: Vec<FileType>,
    pub seconds: f64,
}

#[derive(Debug, Default)]
pub struct FileType {
    pub extension: String,
    pub bytes: u64,
    pub files: u64,
}

impl Snapshot {
    pub fn path(&self, mut id: NodeId) -> PathBuf {
        let mut parts = Vec::new();
        while id != 0 && id != NONE {
            let node = &self.nodes[id as usize];
            parts.push(&node.name);
            id = node.parent;
        }
        let mut path = self.root.clone();
        for name in parts.into_iter().rev() {
            path.push(name);
        }
        path
    }
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        let mut ids = Vec::new();
        let mut next = self.nodes[id as usize].first_child;
        while next != NONE {
            ids.push(next);
            next = self.nodes[next as usize].next_sibling;
        }
        ids
    }
}

pub fn extension(name: &std::ffi::OsStr) -> String {
    std::path::Path::new(name)
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn size_label(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
