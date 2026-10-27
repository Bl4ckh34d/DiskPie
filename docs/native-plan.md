# Native DiskPie implementation plan

Goal: deliver a portable native Windows storage explorer with a shared cross-platform core.
Architecture and constraints: [native-design.md](native-design.md).
Execution: inline in the existing directory; no copy of private legacy data.

1. Completed: default-deny Git exclusions, repository privacy verification and remote connection. Minimal Rust toolchain installed; existing native build prerequisites reused.
2. Completed: failing synthetic scanner/query tests, then flat-tree scanning and bounded background queries. Tests now pass.
3. Completed: scan controls, completion/cancellation, navigation, virtualized tables and bounded donut geometry. Model/chart tests and synthetic native GUI inspection passed.
4. Completed: aggregate-only benchmark mode, portable packaging, dependency notices, README and Windows/Linux/macOS CI. Python/Rust synthetic comparison recorded in native-verification.md.
5. Local release tests, clippy, formatting, privacy checks and GUI smoke passed. Finish with exact staged inventory review, source-only push, CI checks, disposal of build/test intermediates and Obsidian capture.
