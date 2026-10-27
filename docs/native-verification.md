# Native verification — 27 October 2026

## Web interface parity update

The native interface now follows the original web layout and purple/mint palette: fixed navigation and drive cards, scan/exclusion controls, four metrics, a seven-segment donut plus remainder, breadcrumbs, sortable results and 200-row pages. Navigation and legend labels are left aligned. CSV export is an explicit save action; nothing persists automatically. Skipped entries retain actionable paths and reasons, with bounded samples.

- `cargo test --locked --release`: 13 passed, one opt-in performance probe. The probe also passed separately. `cargo fmt --check`, Clippy with warnings denied and all five privacy-guard tests passed.
- Synthetic million-entry index: 26.019 ms; retrieving its final 200-row page: 0.006 ms. Folder queries traverse child links; page changes reuse a compact index. Superseded sorts cancel between bounded chunks and during merging.
- Synthetic 20,000-file scan, three warm repetitions: medians 13.352 ms (one worker), 5.142 ms (four), 4.497 ms (eight). Totals remained exact.
- Windows drive enumeration was exercised independently: four local drives returned valid capacity data. Normal startup enables drive refresh; `--demo` uses synthetic drives and now has a clearly labeled window title.
- Original HTML/CSS/JS and the native preview were compared using synthetic data. The user stopped computer control before the final alignment/interaction recheck; that final visual check remains unverified. No real scan was captured or committed.

The earlier measurements and CI results below describe the initial native delivery, not this update.

## Drive selection correction

Drive selection now switches the entire result view to that location's latest completed scan in this session. A location without a completed scan shows an explicit empty state. Failed or cancelled rescans preserve that location's previous result; a scan finishing after the user selects another drive is cached without stealing the selection. Drive cards show whether a scan is available. Snapshots remain in RAM only and cannot be carried over from a previously running executable when upgrading.

`cargo test --locked --release` passed 17 tests on Windows, with one optional performance probe excluded. New regressions cover switching between two scans, unscanned locations, late completion, cancelled/failed replacement, equivalent Windows drive spellings, and distinct case-sensitive Windows roots. Clippy and formatting checks passed. The preceding layout commit `531cf20` passed all four build/test/package jobs in [CI run 40510070651](https://github.com/Bl4ckh34d/DiskPie/actions/runs/40510070651); the subsequent session-cache correction is locally verified on Windows. Final GUI rechecking remains subject to the computer-control interruption noted above.

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

Windows is locally built and exercised. [CI run 40489772220](https://github.com/Bl4ckh34d/DiskPie/actions/runs/40489772220) passed all four native build/test/package jobs at source commit `bd8f148`: Windows x64, Linux x64, macOS ARM64 and macOS x64. All four archives were uploaded. Linux/macOS GUI behavior has not been manually exercised on hardware. macOS packages are unsigned/unnotarized. Linux archives require compatible system libraries. The initial macOS packaging failure was an omitted upstream license file in `dispatch 0.2.0`; its declared MIT grant and author attribution are now included and the rerun passed.

The public repository contains only an explicit source allowlist. Legacy reports, databases, logs, local paths, scan exports and build artifacts were never staged. All smoke data is synthetic.

After verification, 1.2 GiB of disposable build output and the temporary toolchain installer were removed. The portable distribution and required development toolchain remain available. Normal GUI startup was verified to perform no automatic scan.
