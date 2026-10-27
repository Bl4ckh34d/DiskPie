use diskpie::{
    export::csv_field,
    query::{Sort, View, query_page},
    scan::{Progress, scan_excluding},
};
use std::{
    fs,
    sync::atomic::{AtomicBool, AtomicU64},
};

#[test]
fn exclusion_is_a_directory_boundary_and_counts_descendants() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("skip/deep")).unwrap();
    fs::create_dir_all(t.path().join("skip-other")).unwrap();
    fs::write(t.path().join("skip/deep/private.bin"), [0; 9]).unwrap();
    fs::write(t.path().join("skip-other/keep.bin"), [0; 7]).unwrap();
    let s = scan_excluding(
        t.path(),
        2,
        &AtomicBool::new(false),
        &Progress::default(),
        &[t.path().join("skip")],
    )
    .unwrap();
    assert_eq!((s.files, s.nodes[0].bytes, s.nodes[0].files), (1, 7, 1));
    assert!(!s.nodes.iter().any(|n| n.name == "private.bin"));
}

#[test]
fn pages_sort_all_matches_and_keep_chart_in_size_order() {
    let t = tempfile::tempdir().unwrap();
    for i in 0..405 {
        fs::write(t.path().join(format!("file-{i:04}")), vec![0; i + 1]).unwrap();
    }
    let s = scan_excluding(
        t.path(),
        2,
        &AtomicBool::new(false),
        &Progress::default(),
        &[],
    )
    .unwrap();
    let g = AtomicU64::new(1);
    let p = query_page(&s, &View::Folder(0), Sort::Name, false, 400, 200, &g, 1).unwrap();
    assert_eq!((p.total, p.ids.len()), (405, 5));
    assert_eq!(s.nodes[p.ids[0] as usize].name, "file-0400");
    assert_eq!(s.nodes[p.chart_ids[0] as usize].bytes, 405);
    assert!(query_page(&s, &View::Largest, Sort::Size, true, 0, 200, &g, 2).is_none());
}

#[test]
fn csv_quotes_delimiters_and_neutralizes_spreadsheet_formulas() {
    assert_eq!(csv_field("a,\"b\n"), "\"a,\"\"b\n\"");
    assert_eq!(csv_field("=SUM(A1)"), "\"'=SUM(A1)\"");
    assert_eq!(csv_field(" ordinary"), "\" ordinary\"");
}

#[test]
fn merged_sort_matches_reference_for_every_sort_and_direction() {
    use diskpie::{
        model::{NONE, Node, Snapshot},
        query::query_index,
    };
    let mut s = Snapshot::default();
    for id in 0..10001_u32 {
        s.nodes.push(Node {
            name: format!("sample-{:05}", (id * 7919) % 10001).into(),
            bytes: ((id * 127) % 1000) as u64,
            files: (id % 7) as u64,
            parent: if id == 0 { NONE } else { 0 },
            first_child: if id == 0 { 1 } else { NONE },
            next_sibling: if id == 10000 { NONE } else { id + 1 },
            is_dir: id == 0,
        });
    }
    for sort in [Sort::Name, Sort::Size, Sort::Files] {
        for descending in [false, true] {
            let got = query_index(
                &s,
                &View::Folder(0),
                sort,
                descending,
                &AtomicU64::new(1),
                1,
            )
            .unwrap();
            let mut expected: Vec<u32> = (1..10001).collect();
            expected.sort_by(|a, b| {
                let (l, r) = (&s.nodes[*a as usize], &s.nodes[*b as usize]);
                let c = match sort {
                    Sort::Name => l.name.cmp(&r.name),
                    Sort::Size => l.bytes.cmp(&r.bytes),
                    Sort::Files => l.files.cmp(&r.files),
                };
                (if descending { c.reverse() } else { c })
                    .then_with(|| l.name.cmp(&r.name))
                    .then_with(|| a.cmp(b))
            });
            assert_eq!(got.ids, expected);
            assert_eq!(got.page(9800, 200).ids, expected[9800..]);
        }
    }
}
