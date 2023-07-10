#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

mkdir -p build
pushd build
    cargo build -p rerun_c # TODO(emilk): add this to CMakelists.txt instead?
    cmake -DCMAKE_BUILD_TYPE=Debug ..
    make # VERBOSE=1
popd

./build/example/rerun_example

