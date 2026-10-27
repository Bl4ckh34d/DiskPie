use diskpie::{
    query::{View, query},
    scan::{Progress, scan},
};
use std::{
    fs,
    sync::atomic::{AtomicBool, AtomicU64},
};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("folder/deep")).unwrap();
    fs::create_dir(dir.path().join("empty")).unwrap();
    fs::write(dir.path().join("root.txt"), [0; 5]).unwrap();
    fs::write(dir.path().join("folder/deep/資料.bin"), [0; 23]).unwrap();
    fs::write(dir.path().join("folder/other.bin"), [0; 7]).unwrap();
    dir
}

#[test]
fn nested_sizes_empty_folders_and_paths_are_exact() {
    let dir = fixture();
    for workers in [1, 4] {
        let s = scan(
            dir.path(),
            workers,
            &AtomicBool::new(false),
            &Progress::default(),
        )
        .unwrap();
        assert_eq!((s.files, s.folders, s.nodes.len()), (3, 4, 7));
        assert_eq!(s.nodes[0].bytes, 35);
        assert_eq!(s.children(0).len(), 3);
        let leaf = s.nodes.iter().position(|n| n.name == "資料.bin").unwrap() as u32;
        assert_eq!(
            s.path(leaf),
            fs::canonicalize(dir.path())
                .unwrap()
                .join("folder/deep/資料.bin")
        );
        let folder = s.nodes.iter().find(|n| n.name == "folder").unwrap();
        assert_eq!(folder.bytes, 30);
    }
}

#[test]
fn cancelled_and_invalid_roots_do_not_publish_a_result() {
    let dir = fixture();
    assert!(scan(dir.path(), 2, &AtomicBool::new(true), &Progress::default()).is_err());
    assert!(
        scan(
            &dir.path().join("missing"),
            2,
            &AtomicBool::new(false),
            &Progress::default()
        )
        .is_err()
    );
    assert!(
        scan(
            &dir.path().join("root.txt"),
            2,
            &AtomicBool::new(false),
            &Progress::default()
        )
        .is_err()
    );
}

#[test]
fn rescan_reads_changed_file_sizes() {
    let dir = fixture();
    let first = scan(dir.path(), 2, &AtomicBool::new(false), &Progress::default()).unwrap();
    fs::write(dir.path().join("root.txt"), [0; 50]).unwrap();
    let second = scan(dir.path(), 2, &AtomicBool::new(false), &Progress::default()).unwrap();
    assert_eq!(first.nodes[0].bytes, 35);
    assert_eq!(second.nodes[0].bytes, 80);
}

#[test]
fn queries_bound_results_match_literal_names_and_exact_extensions() {
    let dir = fixture();
    fs::write(dir.path().join("100%_literal.bin.json"), [0; 40]).unwrap();
    let s = scan(dir.path(), 4, &AtomicBool::new(false), &Progress::default()).unwrap();
    let generation = AtomicU64::new(7);
    let largest = query(&s, &View::Largest, 2, &generation, 7).unwrap();
    assert_eq!(largest.total, 4);
    assert_eq!(largest.ids.len(), 2);
    assert_eq!(s.nodes[largest.ids[0] as usize].bytes, 40);
    let found = query(&s, &View::Search("100%_".into()), 10, &generation, 7).unwrap();
    assert_eq!(found.total, 1);
    let bins = query(&s, &View::Extension("bin".into()), 10, &generation, 7).unwrap();
    assert_eq!(bins.total, 2);
    assert_eq!(
        s.types.iter().find(|t| t.extension == "bin").unwrap().bytes,
        30
    );
    assert!(query(&s, &View::Largest, 2, &generation, 6).is_none());
}

#[test]
fn chart_has_bounded_geometry_and_accounts_for_hidden_entries() {
    let slices = diskpie::chart::slices(&[(1, 60), (2, 30), (3, 10)], 120, 2);
    assert_eq!(slices.len(), 3);
    assert_eq!(slices.iter().map(|s| s.bytes).sum::<u64>(), 120);
    assert_eq!(slices[2].id, None);
    assert_eq!(slices[2].bytes, 30);
    assert!(diskpie::chart::slices(&[(1, 0)], 0, 24).is_empty());
}

#[test]
fn cancellation_at_publication_preserves_previous_snapshot() {
    use diskpie::{model::Snapshot, tasks::publish};
    use std::sync::Arc;
    let original = Arc::new(Snapshot {
        files: 7,
        ..Default::default()
    });
    let mut previous = Some(original.clone());
    let result = publish(
        &mut previous,
        Ok(Snapshot::default()),
        &AtomicBool::new(true),
    );
    assert!(result.is_err());
    assert!(Arc::ptr_eq(previous.as_ref().unwrap(), &original));
    assert!(publish(&mut previous, Err("failed".into()), &AtomicBool::new(false)).is_err());
    assert!(Arc::ptr_eq(previous.as_ref().unwrap(), &original));
}

#[test]
fn empty_scan_and_extensionless_files_are_supported() {
    let dir = TempDir::new().unwrap();
    let empty = scan(dir.path(), 0, &AtomicBool::new(false), &Progress::default()).unwrap();
    assert_eq!(
        (empty.nodes[0].bytes, empty.files, empty.folders),
        (0, 0, 1)
    );
    fs::write(dir.path().join("LICENSE"), [0; 9]).unwrap();
    let s = scan(dir.path(), 1, &AtomicBool::new(false), &Progress::default()).unwrap();
    let found = query(
        &s,
        &View::Extension(String::new()),
        10,
        &AtomicU64::new(0),
        0,
    )
    .unwrap();
    assert_eq!(found.total, 1);
    assert_eq!(s.nodes[found.ids[0] as usize].bytes, 9);
}

#[test]
fn background_query_returns_latest_navigation_request() {
    use diskpie::tasks::QueryTask;
    use std::{sync::Arc, time::Duration};
    let dir = fixture();
    let s = Arc::new(scan(dir.path(), 2, &AtomicBool::new(false), &Progress::default()).unwrap());
    let mut task = QueryTask::default();
    task.request(s.clone(), View::Largest);
    task.request(s.clone(), View::Extension("txt".into()));
    loop {
        let (generation, result) = task.receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        if generation == task.current {
            assert_eq!(result.total, 1);
            assert_eq!(s.nodes[result.ids[0] as usize].name, "root.txt");
            break;
        }
    }
}

#[test]
#[ignore = "explicit synthetic performance probe"]
fn million_entry_query_probe() {
    use diskpie::model::{NONE, Node, Snapshot};
    use std::time::Instant;
    let mut s = Snapshot::default();
    for id in 0..1_000_001_u32 {
        s.nodes.push(Node {
            name: format!("synthetic-{id:07}.bin").into(),
            bytes: id as u64,
            parent: if id == 0 { NONE } else { 0 },
            first_child: NONE,
            next_sibling: NONE,
            is_dir: id == 0,
        });
    }
    for view in [View::Largest, View::Search("999".into())] {
        let started = Instant::now();
        let result = query(&s, &view, 10_000, &AtomicU64::new(0), 0).unwrap();
        assert!(result.ids.len() <= 10_000);
        assert!(result.total >= result.ids.len());
        println!(
            "synthetic query {:?}: {:.3} ms; {} matches; {} rows",
            view,
            started.elapsed().as_secs_f64() * 1000.0,
            result.total,
            result.ids.len()
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_cycle_is_skipped() {
    let dir = fixture();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
    let s = scan(dir.path(), 2, &AtomicBool::new(false), &Progress::default()).unwrap();
    assert_eq!((s.nodes[0].bytes, s.skipped), (35, 1));
}

#[cfg(windows)]
#[test]
fn junction_cycle_is_skipped() {
    let dir = fixture();
    let junction = dir.path().join("loop");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(status.status.success());
    let result = scan(dir.path(), 2, &AtomicBool::new(false), &Progress::default());
    fs::remove_dir(&junction).unwrap();
    let s = result.unwrap();
    assert_eq!((s.nodes[0].bytes, s.skipped), (35, 1));
}
