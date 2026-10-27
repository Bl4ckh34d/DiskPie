use crate::model::Snapshot;
use crate::{
    query::{QueryResult, Sort, View, query_index},
    scan::{Progress, scan_excluding},
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
    pub root: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Progress>,
    pub receiver: mpsc::Receiver<Result<Snapshot, String>>,
    pub started: Instant,
}
impl ScanTask {
    pub fn start(root: PathBuf, workers: usize) -> Self {
        Self::start_excluding(root, workers, Vec::new())
    }
    pub fn start_excluding(root: PathBuf, workers: usize, excludes: Vec<PathBuf>) -> Self {
        let requested_root = root.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Progress::default());
        let (tx, receiver) = mpsc::channel();
        let (c, p) = (cancel.clone(), progress.clone());
        std::thread::spawn(move || {
            let result =
                std::panic::catch_unwind(|| scan_excluding(&root, workers, &c, &p, &excludes))
                    .unwrap_or_else(|_| Err("Scanner stopped unexpectedly".into()));
            let _ = tx.send(result);
        });
        Self {
            root: requested_root,
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
    sort: Sort,
    descending: bool,
    offset: usize,
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
            let mut cache: Option<(Arc<Snapshot>, View, Sort, bool, QueryResult)> = None;
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
                let cached = cache
                    .as_ref()
                    .is_some_and(|(snapshot, view, sort, descending, _)| {
                        Arc::ptr_eq(snapshot, &request.snapshot)
                            && *view == request.view
                            && *sort == request.sort
                            && *descending == request.descending
                    });
                if !cached {
                    cache = None;
                    if let Some(index) = query_index(
                        &request.snapshot,
                        &request.view,
                        request.sort,
                        request.descending,
                        &g,
                        request.generation,
                    ) {
                        cache = Some((
                            request.snapshot.clone(),
                            request.view.clone(),
                            request.sort,
                            request.descending,
                            index,
                        ));
                    }
                }
                if g.load(Ordering::Relaxed) == request.generation
                    && let Some((_, _, _, _, index)) = &cache
                {
                    let result = index.page(request.offset, 200);
                    if tx.send((request.generation, result)).is_err() {
                        return;
                    }
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
    pub fn invalidate(&mut self) {
        self.current = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.mailbox.0.lock().unwrap().next = None;
    }
    pub fn request(&mut self, snapshot: Arc<Snapshot>, view: View) {
        self.request_page(snapshot, view, Sort::Size, true, 0);
    }
    pub fn request_page(
        &mut self,
        snapshot: Arc<Snapshot>,
        view: View,
        sort: Sort,
        descending: bool,
        offset: usize,
    ) {
        self.current = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.mailbox.0.lock().unwrap().next = Some(Request {
            snapshot,
            view,
            generation: self.current,
            sort,
            descending,
            offset,
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
