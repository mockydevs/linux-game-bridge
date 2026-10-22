# Contributing

Thanks for helping Linux players get into their games.

1. Open an issue describing the game, Linux distribution, native/Flatpak Steam installation, and actual failure before starting a large feature.
2. Fork the project and create a focused branch.
3. Add behavioral tests for changes to configuration, downloads, extraction, or launch safety. Use synthetic fixtures; never commit proprietary game files, account tokens, private Steam configuration, or credentials.
4. Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
5. Submit a pull request explaining the problem, resulting behavior, verification, and known limits.

Changes to main require a pull request, passing CI, and maintainer review. Direct pushes, force pushes, and branch deletion should remain blocked. Initial development is conservative: one game with a working setup is better than a long unsupported compatibility list.

For installer changes, manually test the official installer with `fetch --installer /absolute/path/to/TruckersMP-Setup.exe` using a temporary `--data-dir`. For launch tests, close your game normally, back up settings, and report the exact game/client/Proton versions. Do not automate account creation, passwords, acceptance of terms, or anti-cheat bypasses.

If you fork this project, preserve the MIT attribution and clearly distinguish your builds from the official upstream release. Downloads of vendor software must remain on official HTTPS endpoints; do not introduce mirrors for proprietary binaries without permission.
