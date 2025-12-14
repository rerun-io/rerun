"""
Test tasks for the recompose project.

These are real tasks used in CI and development workflows.
"""

from pathlib import Path

import recompose

# Project root is two levels up from tasks/
PROJECT_ROOT = Path(__file__).parent.parent.parent


@recompose.task
def test(*, verbose: bool = False, coverage: bool = False) -> recompose.Result[None]:
    """
    Run the pytest test suite.

    Args:
        verbose: Show verbose test output
        coverage: Enable coverage reporting
    """
    recompose.out("Running tests...")

    args = ["uv", "run", "pytest"]

    if verbose:
        args.append("-v")

    if coverage:
        args.extend(["--cov=src/recompose", "--cov-report=term-missing"])

    result = recompose.run(*args, cwd=PROJECT_ROOT)

    if result.failed:
        return recompose.Err(f"Tests failed with exit code {result.returncode}")

    recompose.out("All tests passed!")
    return recompose.Ok(None)
