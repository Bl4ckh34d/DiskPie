# DiskPie contributor rules

- This repository is public. Never commit local filesystem inventories, scan results, real file/folder names, personal paths, databases, logs, screenshots of real scans, credentials, or personal test fixtures.
- `.gitignore` defaults to deny. Stage explicit reviewed source files only. Run `python scripts/check_repository.py --staged` before every commit and check all tracked files before pushing.
- Tests and screenshots use generated synthetic data only. No automatic whole-drive scans, telemetry, network service, or automatic scan persistence. Explicit user-selected CSV page exports are allowed and must remain outside Git.
- Keep scan and query work off the UI thread. Bound worker counts, channels, issue samples, query results, and chart geometry. Virtualize tables. Do not repaint continuously while idle.
- Keep Windows, Linux, and macOS source compatibility. Platform-specific operations belong behind conditional compilation.
- Minimize build artifacts. Keep the portable executable; remove disposable test fixtures and superseded build intermediates after verification.
- Work in this existing directory to avoid copying private legacy artifacts into worktrees. Legacy Python/web files and reports stay ignored and local.
