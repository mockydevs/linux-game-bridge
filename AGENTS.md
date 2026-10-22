# Development rules

- This is an unofficial Linux compatibility assistant. First supported game: ETS2 + official TruckersMP.
- Never redistribute proprietary game/launcher/mod files or collect user credentials.
- Keep downloads on official HTTPS endpoints with certificate validation, size limits and transfer-integrity checks.
- Never start a stale/unverified mod after an update failure. Preserve the last file until its replacement validates.
- Never kill a user's game, force Steam closed, delete saves, or accept third-party terms automatically.
- Steam config changes require a normal Steam shutdown and an owner-only backup. Preserve every unrelated setting.
- No hardcoded home directories. Support Steam libraries on separate drives.
- Before a PR: cargo fmt --all -- --check; cargo clippy --locked --all-targets -- -D warnings; cargo test --locked; cargo audit.
- Main is protected. Contribute through reviewed PRs. Release artifacts are early alpha until clean-machine and wider distro tests are documented.
