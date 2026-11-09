#!/usr/bin/env bash
# Build Support Step Recorder on RHEL 10 (or an EL10 rebuild: CentOS Stream
# 10, AlmaLinux 10, Rocky Linux 10) and package it as a tarball that keeps
# the usual install flow:
#
#   tar xzf ssr-<version>-rhel10-<arch>.tar.gz
#   cd ssr-<version>-rhel10-<arch>
#   sudo ./packaging/install-linux.sh     # udev rule for Wayland input capture
#   ./ssr
#
# Usage (from the repository root):
#   packaging/rhel/build-rhel10.sh [--install-deps] [--offline] [--out DIR]
#
#   --install-deps  dnf install the build dependencies first (needs root;
#                   enables the CodeReady Builder / CRB repository).
#   --offline       build from vendored crates (see --vendor) without network.
#   --vendor        only create vendor/ + .cargo/config.toml for an offline
#                   build on an air-gapped machine, then stop.
#   --out DIR       where to put the tarball (default: dist/).
set -euo pipefail

MIN_RUST="1.95.0"   # egui/eframe 0.37 need Rust 1.95

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/dist"
INSTALL_DEPS=0
OFFLINE=0
VENDOR_ONLY=0

while [ $# -gt 0 ]; do
    case "$1" in
        --install-deps) INSTALL_DEPS=1 ;;
        --offline) OFFLINE=1 ;;
        --vendor) VENDOR_ONLY=1 ;;
        --out) OUT="$2"; shift ;;
        -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

cd "$ROOT"

if [ "$VENDOR_ONLY" -eq 1 ]; then
    say "Vendoring crates for an offline build"
    mkdir -p .cargo
    cargo vendor --locked vendor > .cargo/config.toml
    tar czf "ssr-vendored-source.tar.gz" --exclude=./target --exclude=./dist --exclude=./.git .
    sha256sum ssr-vendored-source.tar.gz | tee ssr-vendored-source.tar.gz.sha256
    say "Copy ssr-vendored-source.tar.gz (+ .sha256) to the build machine, then:"
    echo "    tar xzf ssr-vendored-source.tar.gz && packaging/rhel/build-rhel10.sh --offline"
    exit 0
fi

# ── Platform ─────────────────────────────────────────────────────────
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    case "${VERSION_ID:-}" in
        10*) ;;
        *) echo "warning: built on ${PRETTY_NAME:-unknown}, not an EL10 system;" \
                "the binary may not run on RHEL 10" >&2 ;;
    esac
fi

# ── Build dependencies ───────────────────────────────────────────────
BUILD_DEPS=(
    rust-toolset            # rustc + cargo (Red Hat Developer Tools / AppStream)
    gcc gcc-c++ make
    clang clang-libs        # bindgen (pipewire-sys, libspa-sys)
    pkgconf-pkg-config
    pipewire-devel          # libpipewire-0.3 + libspa-0.2 (screen capture on Wayland)
    dbus-devel              # xdg-desktop-portal (file dialogs, screencast)
    libxkbcommon-devel wayland-devel mesa-libGL-devel mesa-libEGL-devel mesa-libgbm-devel
    libX11-devel libXext-devel libXi-devel libXtst-devel libXrandr-devel libXcursor-devel
    libxcb-devel
)
if [ "$INSTALL_DEPS" -eq 1 ]; then
    say "Installing build dependencies"
    [ "$(id -u)" -eq 0 ] || die "--install-deps needs root (sudo)"
    dnf -y install dnf-plugins-core
    # pipewire-devel and some X11 headers live in CRB on EL10. On RHEL the
    # repository is codeready-builder-for-rhel-10-<arch>-rpms.
    if [ "${ID:-}" = "rhel" ]; then
        subscription-manager repos --enable "codeready-builder-for-rhel-10-$(uname -m)-rpms" || true
    else
        dnf config-manager --set-enabled crb || true
    fi
    dnf -y install "${BUILD_DEPS[@]}"
fi

command -v cargo >/dev/null || die "cargo not found: sudo dnf install rust-toolset (or rerun with --install-deps)"
rust_version="$(rustc --version | awk '{print $2}')"
if [ "$(printf '%s\n%s\n' "$MIN_RUST" "$rust_version" | sort -V | head -1)" != "$MIN_RUST" ]; then
    die "Rust $rust_version is too old (need >= $MIN_RUST). Update rust-toolset, or use
       rustup (https://rustup.rs) on the build machine only."
fi
for pc in libpipewire-0.3 libspa-0.2; do
    pkg-config --exists "$pc" || die "$pc not found: sudo dnf install pipewire-devel (CRB repository)"
done
say "rustc $rust_version · pipewire $(pkg-config --modversion libpipewire-0.3)"

# ── Build ────────────────────────────────────────────────────────────
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
NAME="ssr-${VERSION}-rhel10-${ARCH}"
CARGO_FLAGS=(--release --locked -p support-step-recorder-gui)
[ "$OFFLINE" -eq 1 ] && CARGO_FLAGS+=(--offline)

say "Building $NAME"
cargo build "${CARGO_FLAGS[@]}"
BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/ssr"
[ -x "$BIN" ] || die "binary not found at $BIN"

# ── Package (same layout as the repository: ./ssr + ./packaging/) ────
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/$NAME/packaging"
install -m 0755 "$BIN" "$STAGE/$NAME/ssr"
install -m 0755 packaging/install-linux.sh "$STAGE/$NAME/packaging/install-linux.sh"
install -m 0644 packaging/60-ssr-input.rules "$STAGE/$NAME/packaging/60-ssr-input.rules"
install -m 0644 packaging/rhel/README-RHEL10.md "$STAGE/$NAME/README-RHEL10.md"

# Record what the binary links against, for the runtime package list.
ldd "$BIN" | awk '{print $1}' | sort > "$STAGE/$NAME/packaging/linked-libraries.txt" || true

mkdir -p "$OUT"
tar czf "$OUT/$NAME.tar.gz" -C "$STAGE" "$NAME"
(cd "$OUT" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
say "Done: $OUT/$NAME.tar.gz"
cat "$OUT/$NAME.tar.gz.sha256"
