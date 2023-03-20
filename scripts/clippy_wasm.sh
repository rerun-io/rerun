#!/usr/bin/env bash
# This scripts run clippy on the wasm32-unknown-unknown target,
# using a special clippy_wasm.toml config file which forbids a few more things.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

mv clippy.toml clippy.toml.bak
cp clippy_wasm.toml clippy.toml

function cleanup()
{
    mv clippy.toml.bak clippy.toml
}

trap cleanup EXIT

cargo clippy --version

cargo cranky --all-features --target wasm32-unknown-unknown --target-dir target_wasm -p re_viewer -- --deny warnings
