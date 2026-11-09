#!/usr/bin/env bash
# Build the RHEL 10 tarball inside an EL10 container (podman or docker), from
# any Linux machine. The result lands in dist/.
#
#   packaging/rhel/build-in-container.sh            # CentOS Stream 10 image
#   EL10_IMAGE=docker.io/almalinux:10 packaging/rhel/build-in-container.sh
#
# For a build on a subscribed RHEL 10 host, run build-rhel10.sh directly.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IMAGE="${EL10_IMAGE:-quay.io/centos/centos:stream10}"
ENGINE="${CONTAINER_ENGINE:-$(command -v podman || command -v docker || true)}"
[ -n "$ENGINE" ] || { echo "podman or docker is required" >&2; exit 1; }

# A separate target dir: the host's build artefacts are for another libc.
"$ENGINE" run --rm \
    -v "$ROOT:/src:Z" \
    -w /src \
    -e CARGO_TARGET_DIR=/src/target/rhel10 \
    -e CARGO_HOME=/src/target/rhel10-cargo-home \
    "$IMAGE" \
    bash -c 'packaging/rhel/build-rhel10.sh --install-deps'
