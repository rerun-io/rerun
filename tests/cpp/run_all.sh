#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."

echo "------------ Building all C++ Examples ------------"
/bin/bash ./docs/code_examples/build_all.sh $@

echo "------------ Building & running SDK tests ------------"
/bin/bash ./rerun_cpp/build_and_run_tests.sh $@

echo "------------ Building & running minimal example ------------"
/bin/bash ./examples/cpp/minimal/build_and_run.sh $@

echo "------------ Running roundtrip tests ------------"
python ./tests/roundtrips.py
