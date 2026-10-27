use crate::model::Snapshot;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Latest completed scan per selected root, in memory for this process only.
#[derive(Default)]
pub struct Session {
    selected: PathBuf,
    scans: HashMap<PathBuf, Arc<Snapshot>>,
}

impl Session {
    pub fn select(&mut self, root: &Path) {
        self.selected = key(root);
    }
    pub fn selected(&self, root: &Path) -> bool {
        self.selected == key(root)
    }
    pub fn current(&self) -> Option<Arc<Snapshot>> {
        self.scans.get(&self.selected).cloned()
    }
    pub fn has_scan(&self, root: &Path) -> bool {
        self.scans.contains_key(&key(root))
    }
    pub fn finish(
        &mut self,
        requested: &Path,
        result: Result<Snapshot, String>,
        cancelled: &AtomicBool,
    ) -> Result<(), String> {
        if cancelled.load(Ordering::Relaxed) {
            std::thread::spawn(move || drop(result));
            return Err("Scan cancelled; previous result kept".into());
        }
        let snapshot = Arc::new(result?);
        let old = self.scans.insert(key(requested), snapshot);
        // Free a replaced tree away from the UI thread. Other roots remain cached.
        if old.is_some() {
            std::thread::spawn(move || drop(old));
        }
        Ok(())
    }
}

fn key(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        use std::{
            ffi::OsString,
            os::windows::ffi::{OsStrExt, OsStringExt},
        };
        let mut wide: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        if wide.starts_with(&[92, 92, 63, 92]) {
            wide.drain(..4);
        }
        // Normalize the drive letter, but preserve component case: NTFS may use
        // case-sensitive directories. Keep non-Unicode UTF-16 names intact too.
        for c in &mut wide {
            if *c == 47 {
                *c = 92;
            }
        }
        if wide.len() >= 2 && wide[1] == 58 && (65..=90).contains(&wide[0]) {
            wide[0] += 32;
        }
        if wide.starts_with(&[85, 78, 67, 92]) || wide.starts_with(&[117, 110, 99, 92]) {
            wide.splice(..4, [92, 92]);
        }
        while wide.len() > 3 && wide.last() == Some(&92) {
            wide.pop();
        }
        PathBuf::from(OsString::from_wide(&wide))
    }
    #[cfg(not(windows))]
    {
        absolute
    }
}
