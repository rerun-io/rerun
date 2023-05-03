#!/usr/bin/env bash
# Setup required to build rerun

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

# eframe dependencies needed on run on Linux and Fedora Rawhide:
if [ -x "$(command -v apt-get)" ]; then
    sudo apt-get -y install \
        libatk-bridge2.0 \
        libfontconfig1-dev \
        libfreetype6-dev \
        libglib2.0-dev \
        libgtk-3-dev \
        libssl-dev \
        libxcb-render0-dev \
        libxcb-shape0-dev \
        libxcb-xfixes0-dev \
        libxkbcommon-dev \
        patchelf
elif [ -x "$(command -v dnf)" ];   then
    sudo dnf install \
        clang \
        clang-devel \
        clang-tools-extra \
        libxcb-devel \
        libxkbcommon-devel \
        openssl-devel \
        pkg-config
fi

# Needed to compile and check the code:
rustup install 1.69.0
./scripts/setup_web.sh

echo "setup.sh completed!"
