use crate::model::NodeId;
pub struct Slice {
    pub id: Option<NodeId>,
    pub bytes: u64,
}
pub fn slices(entries: &[(NodeId, u64)], total: u64, limit: usize) -> Vec<Slice> {
    let mut remaining = total;
    let mut slices = Vec::new();
    for &(id, bytes) in entries.iter().take(limit) {
        let bytes = bytes.min(remaining);
        if bytes == 0 {
            continue;
        }
        slices.push(Slice {
            id: Some(id),
            bytes,
        });
        remaining -= bytes;
    }
    if remaining > 0 {
        slices.push(Slice {
            id: None,
            bytes: remaining,
        });
    }
    slices
}
