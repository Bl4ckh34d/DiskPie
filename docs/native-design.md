# DiskPie native desktop design

Approved direction: Rust with egui/eframe, portable Windows first, shared Linux/macOS code. Priorities are metadata scan throughput and responsive navigation. The existing public remote starts empty; private legacy artifacts must never enter Git history.

## Components

- `scan`: bounded directory workers perform fresh metadata enumeration, never read file contents, skip links/reparse points, and send bounded batches to one tree builder. Cancellation preserves the previously completed in-memory result. Errors are counted with capped samples. No automatic persistence or network transport.
- `model`: a flat node arena stores each filename once with parent/sibling IDs, logical byte totals and directory flags. Paths are reconstructed only when needed. Reverse-order aggregation avoids recursion and repeated subtree queries.
- `query`: one background query worker handles navigation, largest files, literal name search and extension filtering. Results are capped and sorted by size; total matches remain visible. Stale query work is cancelled. File type aggregation happens during finalization, not rendering.
- `app`: native folder picker, editable scan root, worker count, progress/cancel, breadcrumbs, virtualized rows, file types, bounded interactive donut chart, path copying and file-manager reveal. A completed scan replaces the previous result; cancellation/failure does not. No delete operation.
- `main`: GUI by default; explicit headless benchmark mode prints aggregate counts only. No local paths or filenames in benchmark output.

## Accuracy and portability

Sizes are logical bytes, not unique allocated disk usage. Hardlinks count per directory entry. Symbolic links, Windows reparse points and cloud placeholders are skipped. Errors mean partial coverage; files can change while scanning. Network OS calls can delay cancellation. The initial scanner uses portable directory enumeration; raw NTFS/MFT access is a later separately measured optimization, not required for this delivery.

Windows ships as an executable with statically linked C runtime. Linux and macOS get native CI build/test jobs and archives. No installed Rust/Python/browser runtime is required for users. Linux system GUI libraries and minimum OS versions remain platform prerequisites.

Queued directories are rechecked for links before enumeration. Since checking and opening a path are separate OS operations, concurrent directory replacement can still race the check. This is a metadata explorer, not a security sandbox. Pending directory paths and the tree scale with the discovered entry count.

## Privacy and storage

No automatic scan on launch, scan database, remembered root, log, crash upload or telemetry. Only explicit clipboard/reveal actions expose selected paths to the local OS. Tests generate disposable synthetic data outside source control. Packaging copies only the executable, public README and upstream dependency license notices. The repository uses a source allowlist, a staged-content privacy check, and a CI repository check. These are defenses in depth; manual staged diff review remains required.

## Validation

Test nested totals, empty directories, file growth, missing roots, cancellation, symlink/junction avoidance, extension matching, bounded queries, chart accounting and publication behavior. Compare old and new scanners on the same generated fixture; report aggregate measurements only. Run release build, Rust tests, formatting/lints, synthetic GUI checks and inspect binary dependencies. Cross-platform CI confirms actual support after pushing.
