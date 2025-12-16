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
from .flows import ci, wheel_test
from .tasks import (
    build_wheel,
    create_test_venv,
    format_check,
    format_code,
    install_wheel,
    lint,
    smoke_test,
    test,
    test_installed,
)

# Suppress unused import warnings - these are used for registration
_ = (
    ci,
    wheel_test,
    build_wheel,
    create_test_venv,
    format_check,
    format_code,
    install_wheel,
    lint,
    smoke_test,
    test,
    test_installed,
)

if __name__ == "__main__":
    recompose.main(python_cmd="uv run python", working_directory="recompose")
