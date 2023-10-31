#!/usr/bin/env bash

# You can pass in extra cmake flags, like:
# -DCMAKE_COMPILE_WARNING_AS_ERROR=ON

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."
set -x

num_threads=$(getconf _NPROCESSORS_ONLN)

mkdir -p build
pushd build
  cmake -DCMAKE_BUILD_TYPE=Debug $@ ..
  cmake --build . --config Debug --target doc_examples -j ${num_threads}
popd
