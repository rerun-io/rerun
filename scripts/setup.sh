#!/usr/bin/env bash
# Setup required to build rerun.
# This file is mirrored in ci_docker/Dockerfile.

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

# C++ SDK requires `apache-arrow` and `cmake`
packagesNeeded='apache-arrow cmake flatbuffers'
if [ -x "$(command -v brew)" ];      then brew install $packagesNeeded
elif [ -x "$(command -v port)" ];    then sudo port install $packagesNeeded
elif [ -x "$(command -v apt-get)" ]; then sudo apt-get -y install $packagesNeeded
elif [ -x "$(command -v dnf)" ];     then sudo dnf install $packagesNeeded
elif [ -x "$(command -v zypper)" ];  then sudo zypper install $packagesNeeded
elif [ -x "$(command -v apk)" ];     then sudo apk add --no-cache $packagesNeeded
elif [ -x "$(command -v winget)" ];  then sudo winget add --no-cache $packagesNeeded
elif [ -x "$(command -v pacman)" ];  then sudo pacman -S $packagesNeeded
else
    echo "FAILED TO INSTALL PACKAGE: Package manager not found. You must manually install: $packagesNeeded">&2;
    exit 1
fi

# Needed to compile and check the code:
rustup install 1.69.0
./scripts/setup_web.sh

echo "setup.sh completed!"
