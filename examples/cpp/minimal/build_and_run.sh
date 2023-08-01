#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../../.."
set -x

num_threads=$(getconf _NPROCESSORS_ONLN)

mkdir -p build
pushd build
    cmake -DCMAKE_BUILD_TYPE=Debug ..
    cmake --build . --config Debug --target rerun_example -j ${num_threads}
popd

./build/examples/cpp/minimal/rerun_example
