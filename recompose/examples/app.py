#!/usr/bin/env python3
"""
Recompose unified entrypoint.

This app combines all tasks and flows for the recompose project.
It serves as THE way to run recompose tasks for both development and CI.

Usage:
    ./run --help
    ./run lint
    ./run format_code
    ./run test
    ./run ci

Inspect flows:
    ./run inspect ci
"""

import recompose

# Import tasks and flows - registers them with recompose
from .flows import ci
from .tasks import format_check, format_code, lint, test

# Suppress unused import warnings - these are used for registration
_ = (ci, format_check, format_code, lint, test)

if __name__ == "__main__":
    recompose.main(python_cmd="uv run python", working_directory="recompose")
