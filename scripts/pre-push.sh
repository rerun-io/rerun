#!/bin/sh

# This script is intended to be called from the git pre-push hook.
# See: hooks/README.md for more details.

# Check if pixi is installed
if ! command -v "pixi" > /dev/null 2>&1; then
  echo "The rerun hooks require 'pixi', which is not installed or not in your PATH. Please run: 'cargo install pixi'."
  exit 1
fi

pixi run fast-lint
