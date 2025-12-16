"""
CI flows for the recompose project.

These flows compose tasks into pipelines that run in CI.
"""

import recompose

from ..tasks import format_check, lint, test


@recompose.flow
def ci() -> None:
    """
    CI pipeline: lint, format check, test, and workflow validation.

    This flow runs:
    1. GHA setup (checkout, python, uv)
    2. lint - Check for code quality issues
    3. format_check - Verify code formatting
    4. test - Run the test suite
    5. generate_gha (check_only) - Ensure workflows are up-to-date

    All checks must pass for CI to succeed.
    """
    # GHA setup steps (no-op when run locally)
    recompose.gha.checkout()
    recompose.gha.setup_python(version="3.12")
    recompose.gha.setup_uv()

    # Run lint and format_check (could run in parallel in future)
    lint()
    format_check()

    # Tests run after lint/format checks pass
    test()

    # Validate that workflow files are up-to-date
    recompose.generate_gha(check_only=True)
