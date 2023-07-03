#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

mkdir -p build
cd build

cargo build -p rerun_c # TODO: add this to CMakelists.txt instead?
cmake -DCMAKE_BUILD_TYPE=Debug ..
make
./rerun_example
