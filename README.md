# Linux Game Bridge

**Run TruckersMP on Linux with Steam Proton — without editing configuration files or copying Windows launchers by hand.**

Linux Game Bridge is an unofficial Rust desktop app and CLI. Its first supported game is **Euro Truck Simulator 2 with TruckersMP**. It uses the official Windows game and multiplayer client through Proton, connecting to the same official servers as Windows players.

**Status: early alpha.** Tested on one Ubuntu desktop with native Steam and an AMD GPU. Broader distribution, hardware, and clean-machine testing is welcome. This is a compatibility assistant, not a native port of TruckersMP.

## Install

Download a Linux x86_64 archive or Debian/Ubuntu package from [Releases](https://github.com/mockydevs/linux-game-bridge/releases).

- **Debian/Ubuntu:** open the `.deb` with your software installer. Alternatively, `sudo apt install ./linux-game-bridge_0.1.1_amd64.deb`.
- **Other supported distributions:** extract the `.tar.gz` and run `./linux-game-bridge`. Use **Application settings → Add Linux Game Bridge to applications menu** to install it for your user.

The downloadable build targets **glibc 2.35 or newer** (Ubuntu 22.04+, Debian 12+, and comparable distributions). You need a working graphical desktop, OpenGL, Vulkan-capable graphics drivers for Proton, and **native Steam**. Flatpak Steam, ARM64, and musl-based distributions are not supported in this first release. Steam installation and GPU drivers remain the distribution's responsibility.

## First drive

1. Install native Steam, sign in, and own/install Euro Truck Simulator 2. Create a [TruckersMP account](https://truckersmp.com/) and meet its current account/game requirements.
2. Open Linux Game Bridge and choose **Prepare Steam**. It backs up Steam's configuration and enables Proton Experimental **only for ETS2**. If necessary, it closes/reopens Steam safely. Let Steam finish downloading the Windows game and dependencies; complete any Steam sign-in/install prompts. Close ETS2 after that first launch.
3. Choose **Install / finish setup**. The app downloads and extracts the official TruckersMP launcher, verifies the multiplayer files, and configures the correct Steam game environment.
4. Choose **Play TruckersMP**, sign in inside the official client, select a server, and drive.

Every play checks the official manifest and downloads changed mod files before starting the game. A failed download or checksum mismatch blocks launch; it does not knowingly start an outdated client. The installed ETS2 version is also checked against TruckersMP's supported version. Game downgrades and Steam beta selections remain explicit actions in Steam.

Steam and TruckersMP handle credentials. The app never asks for your Steam password, stores a login token, or uploads your diagnostics.

## What is automated?

| Task | Handling |
| --- | --- |
| Locate native Steam and game libraries | Automatic, including external drives |
| Enable ETS2's Windows build and Proton | Backed-up Steam setting; Steam handles downloads |
| Obtain the official launcher | Downloaded from TruckersMP over HTTPS; extracted in Rust |
| Multiplayer file updates | Verified before play; changed files replaced atomically |
| Connect game, mod and Proton environment | Automatic |
| Steam/TruckersMP sign-in, terms and account ownership | In the official clients, by the user |
| Anti-cheat or publisher restrictions | Cannot be bypassed by this app |

There is no bundled copy of Steam, Proton, ETS2, TruckersMP, or their proprietary assets. File checks use TruckersMP's published MD5/Squirrel SHA-1 metadata for transfer integrity; these hashes are not publisher signatures. Downloads require HTTPS certificate validation. If the official installer format changes, installation stops with an error.

## CLI

Run without arguments for the desktop window.

```sh
linux-game-bridge doctor                 # read-only local diagnostics
linux-game-bridge doctor --json          # local machine-readable report
linux-game-bridge prepare                # close/reopen Steam if necessary, enable ETS2 Proton
linux-game-bridge install                # download, verify, configure
linux-game-bridge play                   # update, check versions, launch
linux-game-bridge launcher               # open official launcher/settings
linux-game-bridge update                 # verify/repair mod files
linux-game-bridge install-desktop        # per-user application shortcut
```

Use `--steam-root /path/to/Steam` for a nonstandard native Steam installation and `--data-dir /absolute/path` for another app-data location. `fetch --installer /path/to/TruckersMP-Setup.exe` can inspect/install files from an already-downloaded official installer without changing Steam settings or its compatibility prefix.

## Troubleshooting

- **Steam is missing:** install the native Steam package through your distribution, sign in, then return to the app.
- **Prepare never finishes:** check Steam's downloads. The game may have opened normally after its first download; close it before continuing installation.
- **Client protocol mismatch:** close ETS2 and choose **Verify / repair mod**, then Play. No offline/stale-client launch is offered.
- **Unsupported game version:** follow [TruckersMP's official guidance](https://truckersmp.com/knowledge-base) for the supported Steam beta branch. This app does not modify purchased game binaries.
- **Game already running:** use the existing window, or close it normally before setup/updates. The app does not kill your game or force Steam to exit.
- **Graphics or anti-cheat failure:** consult the official client/Proton documentation. These cannot all be fixed by a launcher.

Data lives in `$XDG_DATA_HOME/linux-game-bridge` (usually `~/.local/share/linux-game-bridge`). `launcher.log` contains the official launcher's output. Steam-config backups are owner-only under `backups/`; do not publish those files. The ETS2 prefix lives in the game's Steam library under `steamapps/compatdata/227300`.

Existing TruckersMP launch settings are backed up before this app changes game/mod paths. If you use another launcher for the same Steam prefix, that launcher may need its own paths restored; use one setup manager at a time.

## Build and contribute

Rust **1.88+**, a Linux x86_64 development machine, and the X11/Wayland development packages are needed. On Ubuntu/Debian:

```sh
sudo apt install build-essential pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev
cargo build --locked --release
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

The `Dockerfile` builds on Ubuntu 22.04 with Rust 1.88, avoiding dependencies on a newer developer-machine glibc:

```sh
docker build --target export --output type=local,dest=dist .
./packaging/package.sh dist/linux-game-bridge dist
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [TESTING.md](TESTING.md). Pull requests are welcome. Keep game-specific changes in the Steam/vendor/engine modules; new game support should have a concrete tested setup rather than a generic unsupported promise.

## License and affiliation

The app's source is [MIT licensed](LICENSE), copyright 2026 Mocky Digital. Third-party Rust dependencies retain their respective licenses. Proprietary game and launcher software downloaded at runtime retains its owners' licenses and terms.

This project is not affiliated with or endorsed by TruckersMP, SCS Software, or Valve. Names and trademarks belong to their respective owners.
