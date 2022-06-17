#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."

# Starts a local web-server that serves the contents of the `doc/` folder,
# which is the folder to where the web version is compiled.

cargo install basic-http-server

echo "open http://localhost:9090"

(cd docs && basic-http-server --addr 127.0.0.1:9090 .)
# (cd docs && python3 -m http.server 9090 --bind 127.0.0.1)
