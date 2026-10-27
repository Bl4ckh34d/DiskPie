use crate::model::{FileType, NONE, Node, NodeId, Snapshot, extension};
use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub skipped: AtomicU64,
}

struct Entry {
    name: OsString,
    bytes: u64,
    is_dir: bool,
}
enum Event {
    Batch(NodeId, Vec<Entry>),
    Done,
    Issue(String),
}
type Jobs = (Mutex<VecDeque<(PathBuf, NodeId)>>, Condvar);

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn worker(
    jobs: &Jobs,
    quit: &AtomicBool,
    cancel: &AtomicBool,
    progress: &Progress,
    tx: mpsc::SyncSender<Event>,
) {
    loop {
        let job = {
            let mut queue = jobs.0.lock().unwrap();
            while queue.is_empty()
                && !quit.load(Ordering::Relaxed)
                && !cancel.load(Ordering::Relaxed)
            {
                queue = jobs
                    .1
                    .wait_timeout(queue, Duration::from_millis(100))
                    .unwrap()
                    .0;
            }
            if quit.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) {
                return;
            }
            queue.pop_front()
        };
        let Some((path, parent)) = job else { continue };
        let issue = |path: &Path, error: std::io::Error| {
            let n = progress.errors.fetch_add(1, Ordering::Relaxed);
            if n < 128 {
                let _ = tx.send(Event::Issue(format!("{}: {error}", path.display())));
            }
        };
        // Jobs can wait in the queue: recheck for a replaced link before opening.
        // Path-based enumeration is still not an atomic filesystem sandbox.
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_link(&metadata) => {
                progress.skipped.fetch_add(1, Ordering::Relaxed);
                if tx.send(Event::Done).is_err() {
                    return;
                }
                continue;
            }
            Err(error) => {
                issue(&path, error);
                if tx.send(Event::Done).is_err() {
                    return;
                }
                continue;
            }
            _ => {}
        }
        match fs::read_dir(&path) {
            Err(error) => issue(&path, error),
            Ok(entries) => {
                let mut batch = Vec::with_capacity(256);
                for result in entries {
                    if cancel.load(Ordering::Relaxed) || quit.load(Ordering::Relaxed) {
                        return;
                    }
                    let entry = match result {
                        Ok(e) => e,
                        Err(e) => {
                            issue(&path, e);
                            continue;
                        }
                    };
                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            issue(&entry.path(), e);
                            continue;
                        }
                    };
                    if is_link(&metadata) || (!metadata.is_file() && !metadata.is_dir()) {
                        progress.skipped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let is_dir = metadata.is_dir();
                    let bytes = if is_dir { 0 } else { metadata.len() };
                    if !is_dir {
                        progress.files.fetch_add(1, Ordering::Relaxed);
                        progress.bytes.fetch_add(bytes, Ordering::Relaxed);
                    }
                    batch.push(Entry {
                        name: entry.file_name(),
                        bytes,
                        is_dir,
                    });
                    if batch.len() == 256
                        && tx
                            .send(Event::Batch(
                                parent,
                                std::mem::replace(&mut batch, Vec::with_capacity(256)),
                            ))
                            .is_err()
                    {
                        return;
                    }
                }
                if !batch.is_empty() && tx.send(Event::Batch(parent, batch)).is_err() {
                    return;
                }
            }
        }
        progress.folders.fetch_add(1, Ordering::Relaxed);
        if tx.send(Event::Done).is_err() {
            return;
        }
    }
}

pub fn scan(
    root: &Path,
    workers: usize,
    cancel: &AtomicBool,
    progress: &Progress,
) -> Result<Snapshot, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Scan cancelled".into());
    }
    let started = Instant::now();
    let root = fs::canonicalize(root).map_err(|e| format!("Cannot open location: {e}"))?;
    if !root.is_dir() {
        return Err("Choose a directory or drive".into());
    }
    let mut snapshot = Snapshot {
        root: root.clone(),
        ..Default::default()
    };
    snapshot.nodes.push(Node {
        name: OsString::new(),
        bytes: 0,
        parent: NONE,
        first_child: NONE,
        next_sibling: NONE,
        is_dir: true,
    });
    let jobs = (Mutex::new(VecDeque::from([(root, 0)])), Condvar::new());
    let quit = AtomicBool::new(false);
    let (tx, rx) = mpsc::sync_channel(16);
    let mut pending = 1usize;
    let outcome = std::thread::scope(|scope| {
        for _ in 0..workers.clamp(1, 16) {
            let sender = tx.clone();
            let (jobs_ref, quit_ref) = (&jobs, &quit);
            scope.spawn(move || worker(jobs_ref, quit_ref, cancel, progress, sender));
        }
        drop(tx);
        let result = (|| {
            while pending != 0 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Scan cancelled".to_string());
                }
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Event::Batch(parent, batch)) => {
                        let mut new_jobs = Vec::new();
                        for entry in batch {
                            let id = u32::try_from(snapshot.nodes.len())
                                .ok()
                                .filter(|id| *id != NONE)
                                .ok_or("Too many entries")?;
                            let next_sibling = snapshot.nodes[parent as usize].first_child;
                            snapshot.nodes[parent as usize].first_child = id;
                            if entry.is_dir {
                                new_jobs.push((snapshot.path(parent).join(&entry.name), id));
                                pending += 1;
                            }
                            snapshot.nodes.push(Node {
                                name: entry.name,
                                bytes: entry.bytes,
                                parent,
                                first_child: NONE,
                                next_sibling,
                                is_dir: entry.is_dir,
                            });
                        }
                        if !new_jobs.is_empty() {
                            jobs.0.lock().unwrap().extend(new_jobs);
                            jobs.1.notify_all();
                        }
                    }
                    Ok(Event::Done) => pending -= 1,
                    Ok(Event::Issue(issue)) => snapshot.issues.push(issue),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return Err("Scanner workers disconnected".into()),
                }
            }
            Ok(())
        })();
        quit.store(true, Ordering::Relaxed);
        jobs.1.notify_all();
        // Drop the receiver before joining: cancelled workers may be blocked sending.
        drop(rx);
        result
    });
    outcome?;
    let mut types: HashMap<String, (u64, u64)> = HashMap::new();
    for id in (0..snapshot.nodes.len()).rev() {
        if id % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return Err("Scan cancelled".into());
        }
        let node = &snapshot.nodes[id];
        let (parent, bytes) = (node.parent, node.bytes);
        if node.is_dir {
            snapshot.folders += 1;
        } else {
            snapshot.files += 1;
            let key = extension(&node.name);
            let totals = types.entry(key).or_default();
            totals.0 += bytes;
            totals.1 += 1;
        }
        if parent != NONE {
            snapshot.nodes[parent as usize].bytes += bytes;
        }
    }
    snapshot.types = types
        .into_iter()
        .map(|(extension, (bytes, files))| FileType {
            extension,
            bytes,
            files,
        })
        .collect();
    snapshot.types.sort_unstable_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| a.extension.cmp(&b.extension))
    });
    snapshot.errors = progress.errors.load(Ordering::Relaxed);
    snapshot.skipped = progress.skipped.load(Ordering::Relaxed);
    snapshot.seconds = started.elapsed().as_secs_f64();
    if cancel.load(Ordering::Relaxed) {
        return Err("Scan cancelled".into());
    }
    Ok(snapshot)
}
