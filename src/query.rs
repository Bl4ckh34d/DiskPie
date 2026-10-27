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
    Some(QueryResult { ids, total })
}
