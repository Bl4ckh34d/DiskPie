use crate::model::{NONE, NodeId, Snapshot, extension};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View {
    Folder(NodeId),
    Largest,
    Search(String),
    Extension(String),
}
#[derive(Default)]
pub struct QueryResult {
    pub ids: Vec<NodeId>,
    pub total: usize,
    pub chart_ids: Vec<NodeId>,
}
pub fn query(
    snapshot: &Snapshot,
    view: &View,
    limit: usize,
    generation: &AtomicU64,
    expected: u64,
) -> Option<QueryResult> {
    let mut heap = BinaryHeap::new();
    let mut total = 0;
    let needle = match view {
        View::Search(text) => text.to_lowercase(),
        _ => String::new(),
    };
    let mut accept = |id: NodeId| {
        total += 1;
        if limit == 0 {
            return;
        }
        let key = Reverse((snapshot.nodes[id as usize].bytes, id));
        if heap.len() < limit {
            heap.push(key);
        } else if heap.peek().is_some_and(|smallest| key.0 > smallest.0) {
            heap.pop();
            heap.push(key);
        }
    };
    match view {
        View::Folder(parent) => {
            let mut id = snapshot.nodes[*parent as usize].first_child;
            let mut visited = 0;
            while id != NONE {
                if visited % 4096 == 0 && generation.load(Ordering::Relaxed) != expected {
                    return None;
                }
                accept(id);
                visited += 1;
                id = snapshot.nodes[id as usize].next_sibling;
            }
        }
        _ => {
            for (id, node) in snapshot.nodes.iter().enumerate().skip(1) {
                if id % 4096 == 0 && generation.load(Ordering::Relaxed) != expected {
                    return None;
                }
                let matches = match view {
                    View::Largest => !node.is_dir,
                    View::Search(_) => node.name.to_string_lossy().to_lowercase().contains(&needle),
                    View::Extension(ext) => !node.is_dir && extension(&node.name) == *ext,
                    View::Folder(_) => unreachable!(),
                };
                if matches {
                    accept(id as NodeId);
                }
            }
        }
    }
    if generation.load(Ordering::Relaxed) != expected {
        return None;
    }
    let mut ids: Vec<_> = heap.into_iter().map(|Reverse((_, id))| id).collect();
    ids.sort_unstable_by(|a, b| {
        snapshot.nodes[*b as usize]
            .bytes
            .cmp(&snapshot.nodes[*a as usize].bytes)
            .then_with(|| {
                snapshot.nodes[*a as usize]
                    .name
                    .cmp(&snapshot.nodes[*b as usize].name)
            })
    });
    Some(QueryResult {
        chart_ids: ids.iter().take(7).copied().collect(),
        ids,
        total,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Name,
    Size,
    Files,
}

impl QueryResult {
    pub fn page(&self, offset: usize, limit: usize) -> Self {
        Self {
            ids: self.ids.iter().skip(offset).take(limit).copied().collect(),
            total: self.total,
            chart_ids: self.chart_ids.clone(),
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub fn query_page(
    snapshot: &Snapshot,
    view: &View,
    sort: Sort,
    descending: bool,
    offset: usize,
    limit: usize,
    generation: &AtomicU64,
    expected: u64,
) -> Option<QueryResult> {
    query_index(snapshot, view, sort, descending, generation, expected)
        .map(|index| index.page(offset, limit))
}
pub fn query_index(
    snapshot: &Snapshot,
    view: &View,
    sort: Sort,
    descending: bool,
    generation: &AtomicU64,
    expected: u64,
) -> Option<QueryResult> {
    let cancelled = || generation.load(Ordering::Relaxed) != expected;
    if cancelled() {
        return None;
    }
    let needle = match view {
        View::Search(text) => text.to_lowercase(),
        _ => String::new(),
    };
    let mut ids = Vec::new();
    let mut chart = BinaryHeap::new();
    let mut accept = |id: NodeId| {
        ids.push(id);
        let key = Reverse((snapshot.nodes[id as usize].bytes, id));
        if chart.len() < 7 {
            chart.push(key);
        } else if chart.peek().is_some_and(|min| key.0 > min.0) {
            chart.pop();
            chart.push(key);
        }
    };
    match view {
        View::Folder(parent) => {
            let mut id = snapshot.nodes.get(*parent as usize)?.first_child;
            let mut visited = 0;
            while id != NONE {
                if visited % 4096 == 0 && cancelled() {
                    return None;
                }
                accept(id);
                visited += 1;
                id = snapshot.nodes[id as usize].next_sibling;
            }
        }
        _ => {
            for (id, node) in snapshot.nodes.iter().enumerate().skip(1) {
                if id % 4096 == 0 && cancelled() {
                    return None;
                }
                let matches = match view {
                    View::Largest => !node.is_dir,
                    View::Search(_) => node.name.to_string_lossy().to_lowercase().contains(&needle),
                    View::Extension(ext) => !node.is_dir && extension(&node.name) == *ext,
                    View::Folder(_) => unreachable!(),
                };
                if matches {
                    accept(id as NodeId);
                }
            }
        }
    }
    let compare = |a: &NodeId, b: &NodeId| {
        let (left, right) = (&snapshot.nodes[*a as usize], &snapshot.nodes[*b as usize]);
        let order = match sort {
            Sort::Name => left.name.cmp(&right.name),
            Sort::Size => left.bytes.cmp(&right.bytes),
            Sort::Files => left.files.cmp(&right.files),
        };
        (if descending { order.reverse() } else { order })
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| a.cmp(b))
    };
    // Sort small runs, then merge with cancellation checkpoints. A stale search never
    // monopolizes the single worker for one uncancellable million-entry sort.
    const CHUNK: usize = 2048;
    for chunk in ids.chunks_mut(CHUNK) {
        if cancelled() {
            return None;
        }
        chunk.sort_unstable_by(compare);
    }
    let mut scratch = vec![0; ids.len()];
    let mut width = CHUNK;
    while width < ids.len() {
        for start in (0..ids.len()).step_by(width * 2) {
            let middle = (start + width).min(ids.len());
            let end = (middle + width).min(ids.len());
            let (mut left, mut right) = (start, middle);
            for (out, slot) in scratch.iter_mut().enumerate().take(end).skip(start) {
                if out % 2048 == 0 && cancelled() {
                    return None;
                }
                if left < middle && (right == end || compare(&ids[left], &ids[right]).is_le()) {
                    *slot = ids[left];
                    left += 1;
                } else {
                    *slot = ids[right];
                    right += 1;
                }
            }
        }
        std::mem::swap(&mut ids, &mut scratch);
        width *= 2;
    }
    if cancelled() {
        return None;
    }
    let mut chart_ids: Vec<_> = chart.into_iter().map(|Reverse((_, id))| id).collect();
    chart_ids.sort_unstable_by_key(|id| Reverse(snapshot.nodes[*id as usize].bytes));
    Some(QueryResult {
        total: ids.len(),
        ids,
        chart_ids,
    })
}
