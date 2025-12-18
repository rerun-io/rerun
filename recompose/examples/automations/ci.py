"""
CI automation for the recompose project.

This automation orchestrates the CI pipeline as a multi-job GHA workflow.
"""

import recompose

from ..tasks import lint_all, test


@recompose.automation(
    trigger=recompose.on_push(branches=["main"]) | recompose.on_pull_request(),
)
def ci() -> None:
    """
    CI pipeline: lint_all and test in parallel.

    lint_all combines ruff, mypy, format check, and GHA sync check
    into a single job to reduce container startup overhead.
    """
    recompose.job(lint_all)
    recompose.job(test)
