use diskpie::model::{NONE, Snapshot};
use std::path::PathBuf;
pub fn reveal(path: PathBuf) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let display = path.to_string_lossy();
        let display = if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            display
                .strip_prefix(r"\\?\")
                .unwrap_or(&display)
                .to_string()
        };
        let _ = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{display}\""))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = if path.is_dir() {
            path.as_path()
        } else {
            path.parent().unwrap_or(&path)
        };
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}

pub fn demo_snapshot() -> Snapshot {
    use diskpie::model::{FileType, Node};
    let mut s = Snapshot {
        root: PathBuf::from("Demo"),
        files: 240_000,
        folders: 6,
        seconds: 1.24,
        ..Default::default()
    };
    s.nodes.push(Node {
        name: "Demo".into(),
        bytes: 0,
        files: 5000,
        parent: NONE,
        first_child: 1,
        next_sibling: NONE,
        is_dir: true,
    });
    for (index, (name, gib)) in [
        ("Projects", 42),
        ("Media", 26),
        ("Applications", 18),
        ("Archives", 9),
        ("Documents", 3),
    ]
    .into_iter()
    .enumerate()
    {
        let id = index as u32 + 1;
        let bytes = gib * 1024 * 1024 * 1024;
        s.nodes[0].bytes += bytes;
        s.nodes.push(Node {
            name: name.into(),
            bytes,
            files: 1000,
            parent: 0,
            first_child: NONE,
            next_sibling: if id == 5 { NONE } else { id + 1 },
            is_dir: true,
        });
    }
    for parent in 1..=5 {
        let bytes = s.nodes[parent].bytes;
        let first = s.nodes.len() as u32;
        s.nodes[parent].first_child = first;
        for i in 0..1000 {
            s.nodes.push(Node {
                name: format!("sample-{i:04}.bin").into(),
                bytes: bytes / 1000 + if i == 0 { bytes % 1000 } else { 0 },
                files: 1,
                parent: parent as u32,
                first_child: NONE,
                next_sibling: if i == 999 { NONE } else { first + i + 1 },
                is_dir: false,
            });
        }
    }
    s.files = 5000;
    s.types.push(FileType {
        extension: "bin".into(),
        bytes: s.nodes[0].bytes,
        files: s.files,
    });
    s
}
