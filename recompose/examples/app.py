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

from .flows.ci import ci
from .flows.wheel_test import wheel_test
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

if __name__ == "__main__":
    config = recompose.Config(
        python_cmd="uv run python",
        working_directory="recompose",
    )

    commands = [
        recompose.CommandGroup(
            "Quality",
            [
                lint,
                format_check,
                format_code,
            ],
        ),
        recompose.CommandGroup(
            "Testing",
            [
                test,
            ],
        ),
        recompose.CommandGroup(
            "Build",
            [
                build_wheel,
                create_test_venv,
                install_wheel,
                smoke_test,
                test_installed,
            ],
        ),
        recompose.CommandGroup(
            "Flows",
            [
                ci,
                wheel_test,
            ],
        ),
        recompose.builtin_commands(),
    ]

    recompose.main(config=config, commands=commands)
