"""
CI automation for the recompose project.

This automation orchestrates the CI pipeline as a multi-job GHA workflow.
"""

import recompose

from ..tasks import format_check, lint, test


@recompose.automation(
    trigger=recompose.on_push(branches=["main"]) | recompose.on_pull_request(),
)
def ci() -> None:
    """
    CI pipeline: lint, format check, and test in parallel.

    Each task becomes a separate GHA job that can run in parallel.
    """
    # All three jobs can run in parallel (no dependencies)
    recompose.job(lint)
    recompose.job(format_check)
    recompose.job(test)
