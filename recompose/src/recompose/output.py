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
    "branch": "├──▶",  # Non-last sibling
    "last": "└──▶",  # Last sibling
    "pipe": "│",  # Continuation line
    "parallel": "⊕─┐",  # Parallel group header (corner turns down to children)
    "success": "✓",  # Success
    "failure": "✗",  # Failure
}

# Colors for styled output (Rich style strings)
# These use logical names so the palette can be easily adjusted
COLORS = {
    "tree": "cyan",  # Tree structure symbols (│, ├─▶, etc.)
    "name": "bold",  # Task/automation names
    "success": "green",  # Success status (✓)
    "failure": "red",  # Failure status (✗)
    "success_bold": "bold green",  # Top-level success messages
    "failure_bold": "bold red",  # Top-level failure messages
    "warning": "yellow",  # Warnings
    "dim": "dim",  # Dimmed/secondary text
}

# Prefix widths to align content under headers
BODY_PREFIX = SYMBOLS["pipe"] + " "  # 2 chars: pipe + 1 space (for task body content)
CONTENT_PREFIX = SYMBOLS["pipe"] + "   "  # 4 chars: pipe + 3 spaces (aligns under ├──▶)
LAST_PREFIX = "    "  # 4 chars: 4 spaces (no continuation line)
PARALLEL_PREFIX = "  "  # 2 chars: indent under ⊕─┬

# Task output prefixes (for the recursive capture-and-prefix model)
SUBTASK_MARKER = "\x00SUBTASK:"  # Marker for subtask names in captured output


def _is_status_line(line: str) -> bool:
    """Check if a line is a status line (contains success/failure symbols and timing)."""
    # Status lines contain ✓ or ✗ followed by "succeeded" or "failed" and timing
    return (SYMBOLS["success"] in line or SYMBOLS["failure"] in line) and (
        "succeeded in" in line or "failed in" in line
    )


def prefix_task_output(captured: str) -> str:
    """
    Prefix captured task output with tree symbols.

    - Subtask markers (always plain) → branch header
    - Direct body content → BODY_PREFIX (tighter, before subtasks)
    - Content after subtasks → CONTENT_PREFIX (wider, aligns under ├──▶)
    - Adds blank continuation lines for visual spacing
    """
    if not captured:
        return ""

    lines = captured.rstrip("\n").split("\n")
    result: list[str] = []
    has_seen_subtask = False

    for i, line in enumerate(lines):
        if line.startswith(SUBTASK_MARKER):
            # Add blank line before subtask if there's preceding content (and not already blank)
            if result and result[-1] != SYMBOLS["pipe"]:
                result.append(SYMBOLS["pipe"])
            has_seen_subtask = True
            # Subtask header (marker is always emitted plain)
            name = line[len(SUBTASK_MARKER) :]
            result.append(f"{SYMBOLS['branch']}{name}")
        else:
            # Use tighter prefix before subtasks, wider alignment after
            prefix = CONTENT_PREFIX if has_seen_subtask else BODY_PREFIX
            result.append(f"{prefix}{line}")

            # Add blank line after status lines if more content follows
            if _is_status_line(line) and i < len(lines) - 1:
                result.append(SYMBOLS["pipe"])

    return "\n".join(result)


def print_task_output_styled(prefixed: str, console: Console) -> None:
    """
    Print prefixed task output with styled tree prefixes.

    Tree symbols (│, ├──▶) are printed in cyan.
    Task names (after ├──▶) are printed in bold.
    Content is printed as-is (may contain ANSI colors from child output).
    """
    if not prefixed:
        return

    branch = SYMBOLS["branch"]

    for line in prefixed.split("\n"):
        # Check for branch header line (├──▶task_name)
        if line.startswith(branch):
            # Print branch symbol in tree color, task name in bold
            console.print(branch, style=COLORS["tree"], end="", markup=False, highlight=False)
            console.print(line[len(branch) :], style=COLORS["name"], markup=False, highlight=False)
        elif line.startswith(SYMBOLS["pipe"]):
            # Print pipe prefix in tree color, rest as-is (preserves ANSI)
            console.print(SYMBOLS["pipe"], style=COLORS["tree"], end="", markup=False, highlight=False)
            print(line[len(SYMBOLS["pipe"]) :], flush=True)
        else:
            # No tree prefix, print as-is
            print(line, flush=True)


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

    @property
    def colors_enabled(self) -> bool:
        """Whether color output is enabled."""
        if self._is_gha:
            return False
        return self.console.color_system is not None

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
            self.print(f"\n{SYMBOLS['entry']} ", style=COLORS["tree"], end="")
            self.print(name, style=COLORS["name"])
        else:
            symbol = SYMBOLS["last"] if is_last else SYMBOLS["branch"]
            self.print(symbol, style=COLORS["tree"], end="")
            self.print(name, style=COLORS["name"])

    def print_status(self, success: bool, elapsed: float, prefix: str = "") -> None:
        """Print completion status with optional prefix."""
        if self._is_gha:
            symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
            print(f"{symbol} completed in {elapsed:.2f}s", flush=True)
            print("::endgroup::", flush=True)
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        status_style = COLORS["success"] if success else COLORS["failure"]
        if prefix:
            # Print prefix in header style, then status in success/failure style
            self.print(prefix, style=COLORS["tree"], end="")
            self.print(f"{symbol} {elapsed:.2f}s", style=status_style)
            # Extra blank line with prefix for visual separation
            self.print(prefix.rstrip(), style=COLORS["tree"])
        else:
            self.print(f"{symbol} {elapsed:.2f}s", style=status_style)

    def print_top_level_status(self, name: str, success: bool, elapsed: float) -> None:
        """Print top-level task completion status."""
        if self._is_gha:
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        status = "succeeded" if success else "failed"
        style = COLORS["success_bold"] if success else COLORS["failure_bold"]
        self.print(f"\n{symbol} {name} {status} in {elapsed:.2f}s", style=style)

    def print_parallel_header(self) -> None:
        """Print header for parallel execution group."""
        if self._is_gha:
            return

        self.print(f"{SYMBOLS['parallel']} (parallel)", style=COLORS["tree"])

    def print_automation_header(self, name: str) -> None:
        """Print automation header."""
        if self._is_gha:
            return

        self.print(f"\n{SYMBOLS['entry_down']} ", style=COLORS["tree"], end="")
        self.print(name, style=COLORS["name"])
        self.print(SYMBOLS["pipe"], style=COLORS["tree"])

    def print_automation_status(self, name: str, success: bool, elapsed: float, job_count: int) -> None:
        """Print automation completion status."""
        if self._is_gha:
            return

        symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
        if success:
            msg = f"\n{symbol} {name} completed in {elapsed:.2f}s ({job_count} jobs)"
            self.print(msg, style=COLORS["success_bold"])
        else:
            msg = f"\n{symbol} {name} failed in {elapsed:.2f}s"
            self.print(msg, style=COLORS["failure_bold"])

    def get_continuation_prefix(self, is_last: bool) -> str:
        """Get the prefix for child content based on whether this is the last sibling."""
        return LAST_PREFIX if is_last else CONTENT_PREFIX

    def print_prefixed(self, text: str, prefix: str) -> None:
        """Print text with each line prefixed (prefix styled as header)."""
        if not text:
            return
        for line in text.rstrip("\n").split("\n"):
            if prefix:
                self.print(prefix, style=COLORS["tree"], end="")
            print(line, flush=True)

    def print_error(self, message: str) -> None:
        """Print an error message."""
        if self._is_gha:
            print(f"::error::{message}", flush=True)
        else:
            self.print(f"Error: {message}", style=COLORS["failure_bold"])

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
