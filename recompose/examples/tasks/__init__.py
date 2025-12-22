"""Real tasks for the recompose project."""

from .build import (
    build_wheel,
    create_test_venv,
    install_wheel,
    smoke_test,
    test_installed,
)
from .lint import format_check, format_code, lint, lint_all
from .nested_demo import level_1, level_2a, level_2b, level_3a, level_3b
from .test import test

__all__ = [
    # Lint & format
    "lint",
    "lint_all",
    "format_check",
    "format_code",
    # Test
    "test",
    # Build & distribution
    "build_wheel",
    "create_test_venv",
    "install_wheel",
    "smoke_test",
    "test_installed",
    # Nested demo
    "level_1",
    "level_2a",
    "level_2b",
    "level_3a",
    "level_3b",
]
