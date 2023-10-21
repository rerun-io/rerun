#!/bin/sh

# This script is intended to be called from the git pre-push hook.
# See: hooks/README.md for more details.

# Check if pixi is installed
if ! command -v "pixi" > /dev/null 2>&1; then
  echo "The rerun hooks require 'pixi', which is not installed or not in your PATH. Please run: 'cargo install pixi'."
  exit 1
fi

while read local_ref local_sha remote_ref remote_sha; do
    # Extract the branch name from the local reference
    branch_name=$(echo "$local_ref" | sed 's/^refs\/heads\///')

    # Get the name of the currently active branch
    active_branch=$(git symbolic-ref --short HEAD)

    # Check if the pushed branch matches the active branch
    if [ "$branch_name" = "$active_branch" ]; then
        exec pixi run fast-lint
    else
        echo "Skipping fast-lint because the pushed branch ($branch_name) does not match the active branch ($active_branch)."
    fi
done
