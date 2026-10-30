#!/usr/bin/env bash
# Install the prebuilt cargo-insta release binary for x86_64 Linux into `~/.cargo/bin`.
#
# A version bump means updating both `VERSION` and `SHA256`,
# the latter from the `.sha256` file next to the archive on the release page.

set -eu

VERSION="1.48.0"
SHA256="703aa7d151ae59cac0bf8e7c2c27da4aca8070f3cb178f624ae10621ef77f5f5"
NAME="cargo-insta-x86_64-unknown-linux-musl"

ARCHIVE="$(mktemp)"
trap 'rm -f "$ARCHIVE"' EXIT

curl --fail --silent --show-error --location --output "$ARCHIVE" \
    "https://github.com/mitsuhiko/insta/releases/download/$VERSION/$NAME.tar.xz"
echo "$SHA256  $ARCHIVE" | sha256sum --check
tar -xJf "$ARCHIVE" -C ~/.cargo/bin --strip-components=1 "$NAME/cargo-insta"
