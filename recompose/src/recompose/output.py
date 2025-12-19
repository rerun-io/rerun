"""Unified output management for recompose.

This module provides simple, recursive output formatting for tasks and automations.

The model is simple:
1. Parent prints child's header (├─▶ or └─▶)
2. Parent executes child, capturing ALL output
3. Parent prefixes ALL captured output with continuation prefix
4. Parent prints status with SAME prefix
5. Move to next child

This composes naturally - each level captures and prefixes its children's output.
"""

from __future__ import annotations

import io
import os
import sys
from collections.abc import Generator
from contextlib import contextmanager
from dataclasses import dataclass, field
from enum import Enum

from rich.console import Console


class Verbosity(Enum):
    """Verbosity levels for output."""

    QUIET = 0  # Minimal output (errors only)
    NORMAL = 1  # Standard output (headers, status)
    VERBOSE = 2  # Detailed output (all subprocess output)


# Symbols for tree output
SYMBOLS = {
    "entry": "▶",  # Top-level entry point
    "entry_down": "▼",  # Top-level with children
    "branch": "├─▶",  # Non-last sibling
    "last": "└─▶",  # Last sibling
    "pipe": "│",  # Continuation line
    "parallel": "⊕─┐",  # Parallel group header (corner turns down to children)
    "success": "✓",  # Success
    "failure": "✗",  # Failure
}

# Prefix widths to align content under headers
CONTENT_PREFIX = "│   "  # 4 chars: pipe + 3 spaces
LAST_PREFIX = "    "  # 4 chars: 4 spaces (no continuation line)
PARALLEL_PREFIX = "  "  # 2 chars: indent under ⊕─┬


def prefix_lines(text: str, prefix: str) -> str:
    """Add prefix to each non-empty line of text."""
    if not text:
        return ""
    lines = text.rstrip("\n").split("\n")
    return "\n".join(prefix + line for line in lines)


@dataclass
class OutputManager:
    """
    Simple output manager for recompose.

    Uses a recursive model where each execution level captures child output
    and prefixes it uniformly.
    """

    console: Console = field(default_factory=Console)
    verbosity: Verbosity = Verbosity.NORMAL
    _is_gha: bool = field(default_factory=lambda: os.environ.get("GITHUB_ACTIONS") == "true")

    @property
    def in_gha(self) -> bool:
        """Whether running in GitHub Actions."""
        return self._is_gha

    def print(self, message: str, style: str | None = None, end: str = "\n") -> None:
        """Print a message, optionally with Rich styling."""
        if style and not self._is_gha:
            self.console.print(message, style=style, end=end, markup=False, highlight=False)
        else:
            print(message, end=end, flush=True)

    def print_header(self, name: str, is_last: bool = False, is_top_level: bool = False) -> None:
        """Print a header for a task/job/step."""
        if self._is_gha:
            print(f"::group::{name}", flush=True)
            return

        if is_top_level:
            self.print(f"\n{SYMBOLS['entry']} {name}", style="bold")
        else:
            symbol = SYMBOLS["last"] if is_last else SYMBOLS["branch"]
            self.print(f"{symbol} {name}", style="bold cyan")

    def print_status(self, success: bool, elapsed: float, prefix: str = "") -> None:
        """Print completion status with optional prefix."""
        if self._is_gha:
            symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
            print(f"{symbol} completed in {elapsed:.2f}s", flush=True)
            print("::endgroup::", flush=True)
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        style = "green" if success else "red"
        self.print(f"{prefix}{symbol} {elapsed:.2f}s", style=style)

    def print_top_level_status(self, name: str, success: bool, elapsed: float) -> None:
        """Print top-level task completion status."""
        if self._is_gha:
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        status = "succeeded" if success else "failed"
        style = "bold green" if success else "bold red"
        self.print(f"\n{symbol} {name} {status} in {elapsed:.2f}s", style=style)

    def print_parallel_header(self) -> None:
        """Print header for parallel execution group."""
        if self._is_gha:
            return

        self.print(f"{SYMBOLS['parallel']} (parallel)", style="bold cyan")

    def print_automation_header(self, name: str) -> None:
        """Print automation header."""
        if self._is_gha:
            return

        self.print(f"\n{SYMBOLS['entry_down']} {name}", style="bold blue")
        self.print(SYMBOLS["pipe"])

    def print_automation_status(self, name: str, success: bool, elapsed: float, job_count: int) -> None:
        """Print automation completion status."""
        if self._is_gha:
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        if success:
            self.print(f"\n{symbol} {name} completed in {elapsed:.2f}s ({job_count} jobs)", style="bold green")
        else:
            self.print(f"\n{symbol} {name} failed in {elapsed:.2f}s", style="bold red")

    def get_continuation_prefix(self, is_last: bool) -> str:
        """Get the prefix for child content based on whether this is the last sibling."""
        return LAST_PREFIX if is_last else CONTENT_PREFIX

    def print_prefixed(self, text: str, prefix: str) -> None:
        """Print text with each line prefixed."""
        if text:
            print(prefix_lines(text, prefix), flush=True)

    def print_error(self, message: str) -> None:
        """Print an error message."""
        if self._is_gha:
            print(f"::error::{message}", flush=True)
        else:
            self.print(f"Error: {message}", style="bold red")

    @contextmanager
    def capture_output(self) -> Generator[io.StringIO, None, None]:
        """Context manager to capture stdout/stderr."""
        buffer = io.StringIO()
        old_stdout = sys.stdout
        old_stderr = sys.stderr
        sys.stdout = buffer
        sys.stderr = buffer
        try:
            yield buffer
        finally:
            sys.stdout = old_stdout
            sys.stderr = old_stderr


# Global output manager instance
_output_manager: OutputManager | None = None


def get_output_manager() -> OutputManager:
    """Get the global output manager instance."""
    global _output_manager
    if _output_manager is None:
        _output_manager = OutputManager()
    return _output_manager


def reset_output_manager() -> None:
    """Reset the global output manager (for testing)."""
    global _output_manager
    _output_manager = None


def configure_output(
    verbosity: Verbosity = Verbosity.NORMAL,
    force_color: bool | None = None,
) -> OutputManager:
    """Configure the global output manager."""
    global _output_manager

    if force_color is not None:
        console = Console(force_terminal=force_color)
    else:
        console = Console()

    _output_manager = OutputManager(
        console=console,
        verbosity=verbosity,
    )
    return _output_manager
