#!/usr/bin/env bash
# Setup required to build rerun

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
set -x

(cd "$script_path/example_c" && make run)
(cd "$script_path/example_cpp" && make run)
