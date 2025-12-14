"""
Lint and formatting tasks for the recompose project.

These are real tasks used in CI and development workflows.
"""

from pathlib import Path

import recompose

# Project root is two levels up from tasks/
PROJECT_ROOT = Path(__file__).parent.parent.parent


@recompose.task
def lint() -> recompose.Result[None]:
    """
    Run ruff linter on the codebase.

    Checks for code quality issues without modifying files.
    Used in CI to catch lint errors.
    """
    recompose.out("Running ruff check...")

    result = recompose.run(
        "uv",
        "run",
        "ruff",
        "check",
        "src/",
        "tests/",
        "examples/",
        cwd=PROJECT_ROOT,
    )

    if result.failed:
        return recompose.Err(f"Lint failed with exit code {result.returncode}")

    recompose.out("Lint passed!")
    return recompose.Ok(None)


@recompose.task
def format_check() -> recompose.Result[None]:
    """
    Check code formatting without modifying files.

    Used in CI to verify code is properly formatted.
    Run `format` to apply fixes.
    """
    recompose.out("Checking code formatting...")

    result = recompose.run(
        "uv",
        "run",
        "ruff",
        "format",
        "--check",
        "src/",
        "tests/",
        "examples/",
        cwd=PROJECT_ROOT,
    )

    if result.failed:
        return recompose.Err("Formatting check failed - run 'format' to fix")

    recompose.out("Formatting check passed!")
    return recompose.Ok(None)


@recompose.task
def format() -> recompose.Result[None]:
    """
    Apply code formatting fixes.

    This modifies files in place. Use for local development only,
    not in CI (CI should use format_check instead).
    """
    recompose.out("Applying code formatting...")

    result = recompose.run(
        "uv",
        "run",
        "ruff",
        "format",
        "src/",
        "tests/",
        "examples/",
        cwd=PROJECT_ROOT,
    )

    if result.failed:
        return recompose.Err(f"Formatting failed with exit code {result.returncode}")

    recompose.out("Formatting complete!")
    return recompose.Ok(None)
