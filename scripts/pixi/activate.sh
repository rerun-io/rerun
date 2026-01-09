#!/usr/bin/env bash
# Pixi activation script for Unix.
# Runs ensure-rerun-env to set up the environment.

# ensure-rerun-env may not exist yet on first activation (before package install).
# In that case, silently skip - it will run on next activation after install.
if command -v ensure-rerun-env &> /dev/null; then
    ensure-rerun-env
fi
