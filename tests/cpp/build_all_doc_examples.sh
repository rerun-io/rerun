#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."
set -x

num_threads=$(getconf _NPROCESSORS_ONLN)

mkdir -p build
pushd build
    cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_BUILD_WARNINGS_AS_ERRORS=On ..
    cmake --build . --config Debug --target doc_examples -j ${num_threads}
popd
