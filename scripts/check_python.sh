#!/usr/bin/env bash
# This scripts checks our Python SDK

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

source crates/re_sdk_python/setup_build_env.sh
pip install "crates/re_sdk_python[tests]"
mypy crates/re_sdk_python
pytest crates/re_sdk_python
