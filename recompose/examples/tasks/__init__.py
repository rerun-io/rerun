"""Real tasks for the recompose project."""

from .build import (
    build_wheel,
    create_test_venv,
    install_wheel,
    smoke_test,
    smoke_test_venv,
    test_installed,
    test_installed_venv,
)
from .lint import format_check, format_code, lint
from .test import test
from .virtual_env import Venv

__all__ = [
    # Lint & format
    "lint",
    "format_check",
    "format_code",
    # Test
    "test",
    # Build & distribution
    "build_wheel",
    "create_test_venv",
    "install_wheel",
    "smoke_test",
    "smoke_test_venv",
    "test_installed",
    "test_installed_venv",
    # TaskClasses
    "Venv",
]
