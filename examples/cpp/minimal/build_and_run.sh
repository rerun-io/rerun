#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../../.."
set -x

mkdir -p build
pushd build
    cmake -DCMAKE_BUILD_TYPE=Debug ..
    make -j8 # VERBOSE=1
popd

./build/examples/cpp/minimal/rerun_example
