"""Real tasks for the recompose project."""

from .lint import format, format_check, lint
from .test import test

__all__ = [
    "lint",
    "format_check",
    "format",
    "test",
]
