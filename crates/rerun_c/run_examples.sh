#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
set -x

(cd "$script_path/example_c" && make run)
"$script_path/example_cpp/build_and_run.sh"
