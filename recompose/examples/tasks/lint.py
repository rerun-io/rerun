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
    Run linters (ruff + mypy) on the codebase.

    Checks for code quality and type issues without modifying files.
    Used in CI to catch lint errors.
    """
    # Run ruff check
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
        return recompose.Err(f"Ruff check failed with exit code {result.returncode}")
    recompose.out("Ruff check passed!")

    # Run mypy type check
    recompose.out("Running mypy...")
    result = recompose.run(
        "uv",
        "run",
        "mypy",
        ".",
        cwd=PROJECT_ROOT,
    )
    if result.failed:
        return recompose.Err(f"Mypy failed with exit code {result.returncode}")
    recompose.out("Mypy passed!")

    recompose.out("All checks passed!")
    return recompose.Ok(None)


@recompose.task
def format_check() -> recompose.Result[None]:
    """
    Check code formatting without modifying files.

    Used in CI to verify code is properly formatted.
    Run `format_code` to apply fixes.
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
def format_code() -> recompose.Result[None]:
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


@recompose.task
def lint_all() -> recompose.Result[None]:
    """
    Run all lint checks: ruff, mypy, formatting, and GHA workflow sync.

    This is used in CI to run all static checks in a single job,
    reducing container startup overhead.
    """
    from recompose.builtin_tasks import generate_gha

    recompose.out("Running all lint checks...")

    # Run linters (ruff + mypy)
    result = lint()
    if result.failed:
        return result

    # Check formatting
    result = format_check()
    if result.failed:
        return result

    # Check GHA workflows are in sync
    gha_result = generate_gha(check_only=True)
    if gha_result.failed:
        return recompose.Err("GHA workflows out of sync - run './run generate-gha' to update")

    recompose.out("All lint checks passed!")
    return recompose.Ok(None)
