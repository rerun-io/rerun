#!/usr/bin/env bash
# Run all the setup required to work as a developer in the rerun repository.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

./scripts/setup.sh

# Install
curl -fsSL https://pixi.sh/install.sh | bash

cargo install cargo-cranky # Uses lints defined in Cranky.toml. See https://github.com/ericseppanen/cargo-cranky
cargo install --locked cargo-deny # https://github.com/EmbarkStudios/cargo-deny
cargo install just # https://github.com/casey/just - a command runner
cargo install taplo-cli --locked # https://github.com/tamasfe/taplo - toml formatter/linter/lsp


packagesNeeded='pngcrush pipx clang-format flatbuffers'
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

# ensure pipx is on the path
pipx ensurepath

# install nox for python testing automation
# https://nox.thea.codes/en/stable/
pipx install nox

echo "setup_dev.sh completed!"
