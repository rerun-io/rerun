#!/usr/bin/env bash
# Setup required to build rerun

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

mkdir -p build
cd build

cmake ..
make
./rerun_example
