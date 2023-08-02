#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

num_threads=$(getconf _NPROCESSORS_ONLN)

mkdir -p build
pushd build
    cmake -DCMAKE_BUILD_TYPE=Debug ..
    cmake --build . --config Debug --target rerun_sdk_tests -j ${num_threads}
popd

./build/rerun_cpp/tests/rerun_sdk_tests
