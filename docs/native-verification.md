# Native verification — 27 October 2026

## Windows

- `cargo test --locked`: 9 passed; 1 performance probe intentionally excluded from the default suite.
- `cargo clippy --locked --all-targets -- -D warnings` and `cargo fmt --check`: passed.
- `python scripts/test_privacy.py`: 5 passed. The guard rejects unapproved inventories, private absolute paths (including usernames with spaces), and private staged content even when the working copy has been cleaned.
- Explicit release probe over 1,000,000 synthetic entries: largest-file query 25.508 ms (10,000 displayed rows); literal search 52.285 ms (3,700 matches). Both run off the UI thread.
- Native release GUI inspected with in-memory demo data: folder navigation, file-type view, literal search, bounded chart and aligned table controls. No personal scan screenshots were saved or committed.
- Portable executable imports only Windows system DLLs. No VC runtime DLL, Python, browser or web server is required. Package includes the executable, README and upstream dependency notices.
- Binary inspection found no local user-profile path markers. Package uses explicit file names, not directory-wide copying.

## Synthetic scan measurements

Fixture: 20,000 files in 100 child directories, 10,099,440 logical bytes. Generated in a temporary directory and removed after each benchmark. No personal files were scanned. All scanner totals matched exactly.

Warm local baseline comparison, three interleaved repetitions: previous Python/SQLite scanner median 87.516 ms; initial Rust release median 11.397 ms (1 worker), 4.376 ms (4 workers), 3.606 ms (8 workers).

Final release after adding queued-directory link rechecks, three repetitions: 12.943 ms (1 worker), 4.923 ms (4 workers), 4.397 ms (8 workers). Reproduce the Rust fixture with `python scripts/benchmark.py <executable>`.

These are warm small-fixture measurements, not a whole-drive benchmark or an allocation-size comparison. Whole-drive performance and cold-cache performance have not been measured for this version.

## Review and limits

Independent read-only review found no critical issue. Queued directories are now rechecked for replacement links, but path-based enumeration cannot prevent every concurrent filesystem mutation; this limitation is explicit in the README. Pending directory paths and filename storage scale with the discovered tree. Aborted/failed scans preserve the previous completed result; memory may temporarily contain both.

Windows is locally built and exercised. Linux and macOS have native CI build/test/package jobs; those are separate from local Windows verification. macOS packages are unsigned/unnotarized. Linux archives require compatible system libraries.

The public repository contains only an explicit source allowlist. Legacy reports, databases, logs, local paths, scan exports and build artifacts were never staged. All smoke data is synthetic.
