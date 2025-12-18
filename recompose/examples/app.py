#!/usr/bin/env python3
"""
Recompose unified entrypoint.

This app combines all tasks and automations for the recompose project.
It serves as THE way to run recompose tasks for both development and CI.

Usage:
    ./run --help
    ./run lint
    ./run format-code
    ./run test

Inspect automations:
    ./run inspect --target=ci

Generate GHA workflows:
    ./run generate-gha
"""

import recompose

from .automations import ci
from .tasks import (
    build_wheel,
    format_check,
    format_code,
    lint,
    test,
)

# Create dispatchables for tasks that can be manually triggered
lint_workflow = recompose.make_dispatchable(lint)
test_workflow = recompose.make_dispatchable(
    test,
    inputs={
        "verbose": recompose.BoolInput(default=False, description="Show verbose output"),
        "coverage": recompose.BoolInput(default=False, description="Enable coverage reporting"),
    },
)

# Create the app at module level so subprocess isolation can access it
app = recompose.App(
    python_cmd="uv run python",
    working_directory="recompose",
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
        recompose.builtin_commands(),
    ],
    automations=[ci],
    dispatchables=[lint_workflow, test_workflow],
)

if __name__ == "__main__":
    app.main()
