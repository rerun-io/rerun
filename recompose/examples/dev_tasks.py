#!/usr/bin/env python3
"""
Real development workflow tasks for the recompose project.

These tasks are the actual CI/dev workflow, not just demos.

Run with:
    cd recompose
    uv run python examples/dev_tasks.py --help
    uv run python examples/dev_tasks.py lint
    uv run python examples/dev_tasks.py format-check
    uv run python examples/dev_tasks.py format
    uv run python examples/dev_tasks.py test
"""

from pathlib import Path

import recompose

# Project root is one level up from examples/
PROJECT_ROOT = Path(__file__).parent.parent


@recompose.task
def lint() -> recompose.Result[None]:
    """Run ruff linter on the codebase."""
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
        return recompose.Err(f"Linting failed with exit code {result.returncode}")

    recompose.out("Linting passed!")
    return recompose.Ok(None)


@recompose.task
def format_check() -> recompose.Result[None]:
    """Check code formatting without modifying files."""
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
    """Apply code formatting fixes."""
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
def test(*, verbose: bool = False) -> recompose.Result[None]:
    """Run pytest test suite."""
    recompose.out("Running tests...")

    args = ["uv", "run", "pytest"]
    if verbose:
        args.append("-v")

    result = recompose.run(*args, cwd=PROJECT_ROOT)

    if result.failed:
        return recompose.Err(f"Tests failed with exit code {result.returncode}")

    recompose.out("All tests passed!")
    return recompose.Ok(None)


@recompose.task
def check_all() -> recompose.Result[None]:
    """Run all checks: lint, format-check, and test."""
    recompose.out("Running all checks...")

    # Run lint
    lint_result = recompose.run(
        "uv",
        "run",
        "ruff",
        "check",
        "src/",
        "tests/",
        "examples/",
        cwd=PROJECT_ROOT,
    )
    if lint_result.failed:
        return recompose.Err("Lint check failed")

    # Run format check
    format_result = recompose.run(
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
    if format_result.failed:
        return recompose.Err("Format check failed")

    # Run tests
    test_result = recompose.run("uv", "run", "pytest", cwd=PROJECT_ROOT)
    if test_result.failed:
        return recompose.Err("Tests failed")

    recompose.out("All checks passed!")
    return recompose.Ok(None)


if __name__ == "__main__":
    recompose.main()
