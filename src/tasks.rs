use crate::model::Snapshot;
use crate::{
    query::{QueryResult, View, query},
    scan::{Progress, scan},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};
pub fn publish(
    previous: &mut Option<Arc<Snapshot>>,
    result: Result<Snapshot, String>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        std::thread::spawn(move || drop(result));
        return Err("Scan cancelled; previous result kept".into());
    }
    let old = previous.replace(Arc::new(result?));
    // Releasing millions of filenames must not stall the UI thread.
    if old.is_some() {
        std::thread::spawn(move || drop(old));
    }
    Ok(())
}

pub struct ScanTask {
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Progress>,
    pub receiver: mpsc::Receiver<Result<Snapshot, String>>,
    pub started: Instant,
}
impl ScanTask {
    pub fn start(root: PathBuf, workers: usize) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Progress::default());
        let (tx, receiver) = mpsc::channel();
        let (c, p) = (cancel.clone(), progress.clone());
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| scan(&root, workers, &c, &p))
                .unwrap_or_else(|_| Err("Scanner stopped unexpectedly".into()));
            let _ = tx.send(result);
        });
        Self {
            cancel,
            progress,
            receiver,
            started: Instant::now(),
        }
    }
}
impl Drop for ScanTask {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

struct Request {
    snapshot: Arc<Snapshot>,
    view: View,
    generation: u64,
}
#[derive(Default)]
struct Mailbox {
    next: Option<Request>,
    stop: bool,
}
pub struct QueryTask {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    generation: Arc<AtomicU64>,
    pub receiver: mpsc::Receiver<(u64, QueryResult)>,
    pub current: u64,
}
impl Default for QueryTask {
    fn default() -> Self {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let (tx, receiver) = mpsc::sync_channel(1);
        let (m, g) = (mailbox.clone(), generation.clone());
        std::thread::spawn(move || {
            loop {
                let request = {
                    let mut state = m.0.lock().unwrap();
                    while state.next.is_none() && !state.stop {
                        state = m.1.wait(state).unwrap();
                    }
                    if state.stop {
                        return;
                    }
                    state.next.take().unwrap()
                };
                if let Some(result) = query(
                    &request.snapshot,
                    &request.view,
                    10_000,
                    &g,
                    request.generation,
                ) && tx.send((request.generation, result)).is_err()
                {
                    return;
                }
            }
        });
        Self {
            mailbox,
            generation,
            receiver,
            current: 0,
        }
    }
}
impl QueryTask {
    pub fn request(&mut self, snapshot: Arc<Snapshot>, view: View) {
        self.current = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.mailbox.0.lock().unwrap().next = Some(Request {
            snapshot,
            view,
            generation: self.current,
        });
        self.mailbox.1.notify_one();
    }
}
impl Drop for QueryTask {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.mailbox.0.lock().unwrap().stop = true;
        self.mailbox.1.notify_one();
    }
}
