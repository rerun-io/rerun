#!/usr/bin/env bash

# Installs the prerequisites for compiling Rerun to Wasm for the web.
# This file is referred to directly by documentation, so do not move it without updating those docs!

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

# For compiling to Wasm:
rustup target add wasm32-unknown-unknown

# For generating JS bindings:
# cargo install wasm-bindgen-cli --version 0.2.84
# We use our own patched version containing this critical fix: https://github.com/rustwasm/wasm-bindgen/pull/3310
cargo install wasm-bindgen-cli --git https://github.com/rerun-io/wasm-bindgen.git --rev 13283975ddf48c2d90758095e235b28d381c5762

# For local tests with `start_server.sh`:
# cargo install basic-http-server

# binaryen gives us wasm-opt, for optimizing the an .wasm file for speed and size
packagesNeeded='binaryen'
if [ -x "$(command -v brew)" ];      then brew install $packagesNeeded
elif [ -x "$(command -v apt-get)" ]; then sudo apt-get -y install $packagesNeeded
elif [ -x "$(command -v dnf)" ];     then sudo dnf install $packagesNeeded
elif [ -x "$(command -v zypper)" ];  then sudo zypper install $packagesNeeded
elif [ -x "$(command -v apk)" ];     then sudo apk add --no-cache $packagesNeeded
elif [ -x "$(command -v winget)" ];  then sudo winget add --no-cache $packagesNeeded
else
    echo "FAILED TO INSTALL PACKAGE: Package manager not found. You must manually install: $packagesNeeded">&2;
    exit 1
fi
