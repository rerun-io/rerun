#!/usr/bin/env bash
# Run all the setup required to work as a developer in the rerun repository.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

./setup.sh
cargo install cargo-cranky # Uses lints defined in Cranky.toml. See https://github.com/ericseppanen/cargo-cranky
cargo install cargo-deny # https://github.com/EmbarkStudios/cargo-deny

echo "setup_dev.sh completed!"
