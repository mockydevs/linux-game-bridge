# Verification for 0.1.1 alpha

The first release is a working alpha, not a claim that every Linux distribution or Windows game is supported.

## Automated checks

- `cargo test --locked`: 16 behavioral tests covering Steam mapping preservation/idempotence, external library paths, correct prefix selection, PE version reading, Windows path conversion, installer extraction (including BOM and signed-PE trailing bytes), corrupt download handling, traversal/symlink rejection, repairing launcher resources, and launcher settings preservation/error handling.
- `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo audit`: no known vulnerabilities at the time of verification. It reports **RUSTSEC-2026-0192**, a maintenance warning for `ttf-parser` through the GUI font stack. This warning is recorded rather than suppressed; the UI loads bundled fonts and has no font-upload feature. Updating the GUI font stack is a follow-up task.
- Ubuntu 22.04 / Rust 1.88 container build for the distributed executable.

## Real integration checks

On the development Ubuntu x86_64 desktop, with native Steam, an AMD Radeon GPU, and a purchased ETS2 installation:

1. Read-only `doctor --json` correctly found native Steam, the game, Proton Experimental, runtime 4, and the existing ETS2 prefix.
2. `fetch --installer` extracted the real official TruckersMP Squirrel installer, verified its package metadata, and downloaded/verified all 145 required mod files.
3. `install` configured the official launcher in the real game prefix, preserving/backing up existing settings.
4. `play` verified the current manifest and supported ETS2 version, then started the official launcher and mod. TruckersMP logged client, D3D11, UI, keyboard and mouse initialization successfully.
5. The desktop window opened and exited successfully in a smoke run.

The earlier manual prototype on this same machine was used by the owner to sign in and play on the official multiplayer service. The Rust app reuses that verified approach; a fresh authenticated server-join session through this exact packaged build still needs user testing.

## Not yet verified

- A completely clean computer, fresh Steam account, Steam Guard dialogs, or a new game purchase.
- Full GUI interaction and visual QA across desktop environments and display scaling settings.
- NVIDIA/Intel-only graphics, additional Linux distributions, Flatpak Steam, ARM64, or Steam Deck controls.
- Unattended recovery from every Steam or publisher update, unsupported anti-cheat, and any game beyond ETS2/TruckersMP.

Do not report an untested path as supported. Report failures through GitHub Issues with the distro, app/game/client/Proton versions and relevant errors; omit private Steam configuration and credentials.

## Debian runtime correction

A minimal Ubuntu 22.04 container exposed missing X11 runtime dependencies that were already installed on the development desktop. The package now declares X11/XCB, cursor/input, XKB and EGL libraries explicitly. After adding exactly those dependencies, the packaged executable completed its headless graphical smoke test with exit code 0. The missing-display CLI hint now uses the correct executable name.
