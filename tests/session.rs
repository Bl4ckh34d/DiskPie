use diskpie::{
    model::{NONE, Node, Snapshot},
    session::Session,
};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

fn result(root: &str, bytes: u64) -> Snapshot {
    Snapshot {
        root: root.into(),
        nodes: vec![Node {
            name: root.into(),
            bytes,
            files: 1,
            parent: NONE,
            first_child: NONE,
            next_sibling: NONE,
            is_dir: true,
        }],
        files: 1,
        folders: 1,
        ..Default::default()
    }
}

#[test]
fn selecting_a_drive_restores_its_scan_and_unscanned_drives_are_empty() {
    let mut session = Session::default();
    let cancel = AtomicBool::new(false);
    session.select(Path::new("drive-a"));
    session
        .finish(Path::new("drive-a"), Ok(result("drive-a", 10)), &cancel)
        .unwrap();
    let first = session.current().unwrap();
    session.select(Path::new("drive-b"));
    assert!(session.current().is_none());
    session
        .finish(Path::new("drive-b"), Ok(result("drive-b", 20)), &cancel)
        .unwrap();
    session.select(Path::new("drive-a"));
    assert!(Arc::ptr_eq(&first, &session.current().unwrap()));
    assert_eq!(session.current().unwrap().nodes[0].bytes, 10);
    session.select(Path::new("drive-b"));
    assert_eq!(session.current().unwrap().nodes[0].bytes, 20);
}

#[test]
fn finishing_a_background_drive_does_not_switch_the_selected_drive() {
    let mut session = Session::default();
    let cancel = AtomicBool::new(false);
    session.select(Path::new("drive-b"));
    session
        .finish(Path::new("drive-a"), Ok(result("drive-a", 10)), &cancel)
        .unwrap();
    assert!(session.current().is_none());
    session.select(Path::new("drive-a"));
    session
        .finish(Path::new("drive-a"), Err("read error".into()), &cancel)
        .unwrap_err();
    assert_eq!(session.current().unwrap().nodes[0].bytes, 10);
    session
        .finish(
            Path::new("drive-a"),
            Ok(result("drive-a", 99)),
            &AtomicBool::new(true),
        )
        .unwrap_err();
    assert_eq!(session.current().unwrap().nodes[0].bytes, 10);
}

#[cfg(windows)]
#[test]
fn drive_letter_and_verbatim_spellings_share_the_same_scan() {
    let mut session = Session::default();
    session.select(Path::new(r"X:\"));
    session
        .finish(
            Path::new(r"x:/"),
            Ok(result("synthetic", 17)),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(session.current().unwrap().nodes[0].bytes, 17);
    session.select(Path::new(r"\\?\X:\"));
    assert_eq!(session.current().unwrap().nodes[0].bytes, 17);
}

#[cfg(windows)]
#[test]
fn case_sensitive_roots_do_not_replace_each_other() {
    let mut session = Session::default();
    let upper = Path::new(r"Y:\synthetic\Upper");
    let lower = Path::new(r"Y:\synthetic\upper");
    session
        .finish(
            upper,
            Ok(result("upper-case-root", 10)),
            &AtomicBool::new(false),
        )
        .unwrap();
    session
        .finish(
            lower,
            Ok(result("lower-case-root", 20)),
            &AtomicBool::new(false),
        )
        .unwrap();
    session.select(upper);
    assert_eq!(session.current().unwrap().nodes[0].bytes, 10);
    session.select(lower);
    assert_eq!(session.current().unwrap().nodes[0].bytes, 20);
}
