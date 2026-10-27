# DiskPie

A native, private disk usage explorer built with Rust and egui/eframe.
Windows portable comes first. The same scanner and interface build for Linux and macOS.

## Windows portable

Unzip the Windows build and double-click **DiskPie.exe**. No installer, administrator access,
Python, browser, local server, or installed Rust runtime is required. The app is currently unsigned.

1. Browse to a directory or enter a drive/folder path, then select **Scan location**.
2. Navigate through the donut chart, folder names or breadcrumbs.
3. Switch to **Largest files**, **File types** or literal name search.
4. **Copy** copies a full path; **Show** reveals an entry in the system file manager.
5. **Stop scan** preserves the previous completed result. Closing the app discards results.

The scanner only reads metadata. DiskPie never deletes files. It does not automatically scan,
save inventories, remember locations, write logs, upload data, or start a network listener.
Clipboard and file-manager actions happen only when selected. Legacy web reports are never imported.

## Performance

- Bounded background directory workers (1–16, default 4); try 1–2 for HDD/network storage and 4–8 for SSDs.
- Flat in-memory tree, filenames stored once, iterative subtree totals. No database or JSON transport in the scan path.
- Background search and ranking with cancellation of stale queries. Tables draw visible rows only.
- At most 10,000 rows per view; the total matching count is shown. Search narrows large folders.
- The donut renders at most 24 entries plus an exact remainder; no per-file geometry for a whole drive.
- Progress refreshes at 10 Hz. Idle windows do not continuously repaint.

The portable scanner uses ordinary directory enumeration without elevated raw-volume access.
It does not yet use an NTFS MFT/USN fast path. Performance depends on storage and filesystem caches.
Memory scales with entry count; a new scan temporarily coexists with the previous result.

## Accuracy

Sizes are **logical file lengths**, not physical allocated space. Sparse/compressed files and
hardlinks can differ from OS drive usage; hardlinks count once per directory entry. Symlinks,
Windows junctions/reparse points (including cloud placeholders), and special files are skipped.
An explicitly selected linked root is resolved; links inside it are not traversed.
Link skipping is best-effort on a changing filesystem: another process can replace a directory
between checking metadata and opening it. Path-based scanning is not a security sandbox.

Read errors produce a partial result and visible counters (up to 128 error samples).
Files may change during a scan. Every rescan reads metadata afresh. A blocked filesystem call
can delay cancellation; the UI remains responsive. Traversal does not need administrator rights.

## Build and test

Install stable Rust. Windows needs the MSVC C++ tools and Windows SDK.
Linux needs native X11/Wayland/OpenGL build packages and a desktop portal for the folder picker.
macOS needs Xcode Command Line Tools.

```text
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

Windows packaging: `powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1`.
The resulting `dist/DiskPie-windows-x64.zip` contains the executable, this README and dependency license notices.
`DiskPie.cmd` starts the locally packaged executable after a build.

CI runs tests and creates portable archives on Windows, Linux and macOS. Linux archives need
compatible system GUI libraries; they are not universal static binaries. macOS `.app` bundles
are unsigned and unnotarized. Intel and Apple Silicon builds are separate.

Optional commands:

```text
DiskPie --demo
DiskPie --scan <directory>
DiskPie --benchmark <directory> [workers]
```

Demo uses synthetic in-memory data. Benchmark mode is explicit and prints aggregate counts/timing
only, never filenames or paths. Windows GUI executables should be launched from a parent process
that captures stdout when benchmarking. The old web app's persistent snapshots and CSV exports are
not part of this version.

## Repository privacy

This repository is source-only and uses a default-deny `.gitignore`. Do not force-add reports,
databases, logs, screenshots of real scans, or personal test data. Tests create synthetic temporary
fixtures and remove them afterwards. Review staged diffs and run:

```text
python scripts/check_repository.py --staged
```

The check verifies Git index blobs against an explicit source allowlist and common private-data
patterns. It supplements manual review; it cannot recognize every possible personal filename.
Build output and legacy local files stay outside Git. No telemetry or upload feature exists.
