#!/usr/bin/env bash
# Setup required to build rerun

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

# eframe dependencies needed on run on Linux and Fedora Rawhide:
if [ -x "$(command -v apt-get)" ]; then sudo apt-get -y install $(grep -o '^[^#]*' apt-packages.txt)
elif [ -x "$(command -v dnf)" ];   then sudo dnf install $(grep -o '^[^#]*' dnf-packages.txt)
fi

# Needed to compile and check the code:
rustup install 1.64.0
./scripts/setup_web.sh

echo "setup.sh completed!"
