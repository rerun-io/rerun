"""
CI flows for the recompose project.

These flows compose tasks into pipelines that run in CI.
"""

import recompose

from ..tasks import format_check, lint, test


@recompose.flow
def ci() -> None:
    """
    CI pipeline: lint, format check, and test.

    This flow runs:
    1. GHA setup (checkout, python, uv)
    2. lint - Check for code quality issues
    3. format_check - Verify code formatting
    4. test - Run the test suite

    All checks must pass for CI to succeed.
    """
    # GHA setup steps (no-op when run locally)
    recompose.gha.checkout.flow()
    recompose.gha.setup_python(version="3.12").flow()
    recompose.gha.setup_uv().flow()

    # Run lint and format_check (could run in parallel in future)
    lint.flow()
    format_check.flow()

    # Tests run after lint/format checks pass
    test.flow()
