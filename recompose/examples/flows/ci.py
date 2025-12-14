"""
CI flows for the recompose project.

These flows compose tasks into pipelines that run in CI.
"""

# isort: off
import recompose

from tasks import format_check, lint, test  # noqa: F401 - registers tasks
# isort: on


@recompose.flow
def ci() -> None:
    """
    CI pipeline: lint, format check, and test.

    This flow runs:
    1. lint - Check for code quality issues
    2. format_check - Verify code formatting
    3. test - Run the test suite

    All checks must pass for CI to succeed.
    """
    # Run lint and format_check (could run in parallel in future)
    lint.flow()
    format_check.flow()

    # Tests run after lint/format checks pass
    test.flow()
