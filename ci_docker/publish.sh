#!/usr/bin/env bash
set -eux

VERSION=0.19.0 # Bump on each new version.

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Read the pinned Rust toolchain channel from `rerun/rust-toolchain` and bake
# that exact version into the image.
RUST_TOOLCHAIN="$(grep -E '^[[:space:]]*channel[[:space:]]*=' \
  "$REPO_ROOT/rust-toolchain" \
  | head -n1 \
  | sed -E 's/.*=[[:space:]]*"([^"]+)".*/\1/')"

if [[ -z "$RUST_TOOLCHAIN" ]]; then
  echo "Failed to parse channel from $REPO_ROOT/rust-toolchain" >&2
  exit 1
fi

echo "Building ci_docker:$VERSION with pre-installed Rust $RUST_TOOLCHAIN"

# The build needs to run from top of repo to access the requirements.txt
cd "$REPO_ROOT/rerun"

# Build and push the image to GitHub Container Registry
# buildx wants to do all of this in one step
docker buildx build --pull --platform linux/arm64,linux/amd64 \
  --build-arg "VERSION=$VERSION" \
  --build-arg "RUST_TOOLCHAIN=$RUST_TOOLCHAIN" \
  -t ghcr.io/rerun-io/ci_docker \
  -t ghcr.io/rerun-io/ci_docker:$VERSION \
  --push -f ci_docker/Dockerfile .
