"""Real tasks for the recompose project."""

from .build import build_wheel, create_test_venv, install_wheel, smoke_test, test_installed
from .lint import format, format_check, lint
from .test import test
from .workflows import update_workflows, validate_workflows

__all__ = [
    # Lint & format
    "lint",
    "format_check",
    "format",
    # Test
    "test",
    # Build & distribution
    "build_wheel",
    "create_test_venv",
    "install_wheel",
    "smoke_test",
    "test_installed",
    # Workflow management
    "update_workflows",
    "validate_workflows",
]
