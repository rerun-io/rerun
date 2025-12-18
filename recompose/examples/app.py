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
from .flows.wheel_test import wheel_test, wheel_test_v2
from .tasks import (
    build_wheel,
    format_check,
    format_code,
    lint,
    test,
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
            ],
        ),
        recompose.CommandGroup(
            "Flows",
            [
                ci,
                wheel_test,
                wheel_test_v2,
            ],
        ),
        recompose.builtin_commands(),
    ]

    recompose.main(config=config, commands=commands)
