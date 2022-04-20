#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"

packagesNeeded='binaryen jq'
if [ -x "$(command -v brew)" ];      then brew install $packagesNeeded
elif [ -x "$(command -v apt-get)" ]; then sudo apt-get install $packagesNeeded
elif [ -x "$(command -v dnf)" ];     then sudo dnf install $packagesNeeded
elif [ -x "$(command -v zypper)" ];  then sudo zypper install $packagesNeeded
elif [ -x "$(command -v apk)" ];     then sudo apk add --no-cache $packagesNeeded
else echo "FAILED TO INSTALL PACKAGE: Package manager not found. You must manually install: $packagesNeeded">&2; fi

# Extra eframe dependencies needed on Linux and Fedora Rawhide:
if [ -x "$(command -v apt-get)" ]; then sudo apt-get install libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libspeechd-dev libxkbcommon-dev libssl-dev
elif [ -x "$(command -v dnf)" ];   then sudo dnf install clang clang-devel clang-tools-extra speech-dispatcher-devel libxkbcommon-devel pkg-config openssl-devel libxcb-devel
fi

set -x

rustup install 1.60.0
cargo install cargo-deny
./viewer/setup_web.sh
./viewer/build_web.sh
