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
    format_check,
    format_code,
    lint,
    test,
)

# Create the app at module level so subprocess isolation can access it
app = recompose.App(
    config=recompose.Config(
        python_cmd="uv run python",
        working_directory="recompose",
    ),
    commands=[
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
            ],
        ),
        recompose.builtin_commands(),
    ],
)

if __name__ == "__main__":
    app.main()
