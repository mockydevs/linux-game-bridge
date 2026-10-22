#!/bin/sh
set -eu
binary=$(realpath "${1:?Usage: package.sh /path/to/linux-game-bridge [output-dir]}")
output=$(realpath -m "${2:-dist}")
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
version=$("$binary" --version | awk '{print $2}')
case "$version" in *[!0-9.]*|'') echo 'Unexpected version' >&2; exit 1;; esac
mkdir -p "$output"
stage=$(mktemp -d)
trap 'rm -r "$stage"' EXIT HUP INT TERM
name="linux-game-bridge-${version}-linux-x86_64"
mkdir -p "$stage/$name"
install -m 755 "$binary" "$stage/$name/linux-game-bridge"
cp "$root/LICENSE" "$root/README.md" "$root/THIRD_PARTY_NOTICES.txt" "$stage/$name/"
tar -C "$stage" -czf "$output/$name.tar.gz" "$name"
if command -v dpkg-deb >/dev/null 2>&1; then
  pkg="$stage/deb"
  mkdir -p "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/share/applications" "$pkg/usr/share/doc/linux-game-bridge"
  install -m 755 "$binary" "$pkg/usr/bin/linux-game-bridge"
  cp "$root/LICENSE" "$pkg/usr/share/doc/linux-game-bridge/copyright"
  cp "$root/THIRD_PARTY_NOTICES.txt" "$pkg/usr/share/doc/linux-game-bridge/THIRD_PARTY_NOTICES.txt"
  cat > "$pkg/DEBIAN/control" <<CONTROL
Package: linux-game-bridge
Version: $version
Architecture: amd64
Maintainer: Mocky Digital <mockydevs@users.noreply.github.com>
Depends: libc6 (>= 2.35), libx11-6, libxkbcommon0, libgl1, libwayland-client0
Section: games
Priority: optional
Homepage: https://github.com/mockydevs/linux-game-bridge
Description: Unofficial TruckersMP setup assistant for Linux
 Rust desktop app to configure Steam Proton, install the official
 TruckersMP launcher, verify updates, and launch ETS2 multiplayer.
 Native Steam and a purchased copy of ETS2 are required.
CONTROL
  cat > "$pkg/usr/share/applications/linux-game-bridge.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Linux Game Bridge
Comment=Run TruckersMP on Linux with Steam Proton
Exec=linux-game-bridge
Icon=applications-games
Terminal=false
Categories=Game;Simulation;
DESKTOP
  dpkg-deb --root-owner-group --build "$pkg" "$output/linux-game-bridge_${version}_amd64.deb"
fi
(cd "$output" && sha256sum "$name.tar.gz" ./*.deb 2>/dev/null > SHA256SUMS) || (cd "$output" && sha256sum "$name.tar.gz" > SHA256SUMS)
printf 'Packages written to %s\n' "$output"
