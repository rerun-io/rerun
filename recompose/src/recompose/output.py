"""Unified output management for recompose.

This module provides a centralized OutputManager that handles all output formatting
for tasks, automations, and steps with consistent styling and hierarchy.

Features:
- Hierarchical tree-style output for nested tasks and automations
- Rich console colors and styling (local mode)
- GHA ::group:: markers (CI mode)
- Consistent symbols and timing display
"""

from __future__ import annotations

import os
import sys
import time
from collections.abc import Callable, Generator
from contextlib import contextmanager
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, TextIO

from rich.console import Console


class Verbosity(Enum):
    """Verbosity levels for output."""

    QUIET = 0  # Minimal output (errors only)
    NORMAL = 1  # Standard output (headers, status)
    VERBOSE = 2  # Detailed output (all subprocess output)


# Symbols for tree output
SYMBOLS = {
    "entry": "\u25bc",  # Top-level entry point (▼)
    "branch": "\u251c\u2500\u25b6",  # Sequential item (├─▶)
    "last": "\u2514\u2500\u25b6",  # Last item (└─▶)
    "pipe": "\u2502",  # Continuation line (│)
    "parallel_start": "\u2295\u2500\u252c\u2500\u25b6",  # Parallel fork (⊕─┬─▶)
    "parallel_branch": "\u2502 \u251c\u2500\u25b6",  # Parallel item (│ ├─▶)
    "parallel_last": "\u2502 \u2514\u2500\u25b6",  # Last parallel item (│ └─▶)
    "success": "\u2713",  # Success (✓)
    "failure": "\u2717",  # Failure (✗)
}


@dataclass
class ScopeInfo:
    """Information about a nested scope (task/step/job)."""

    name: str
    kind: str  # "task", "step", "job", "parallel"
    start_time: float
    is_last: bool = False

    @property
    def elapsed(self) -> float:
        """Elapsed time since scope started."""
        return time.perf_counter() - self.start_time


class PrefixWriter:
    """Wrapper that adds tree-style prefix to output lines."""

    def __init__(self, wrapped: TextIO, get_prefix: Callable[[], str]):
        self._wrapped = wrapped
        self._get_prefix = get_prefix
        self._at_line_start = True

    def write(self, s: str) -> int:
        if not s:
            return 0

        result = []
        for char in s:
            if self._at_line_start and char != "\n":
                result.append(self._get_prefix())
                self._at_line_start = False
            result.append(char)
            if char == "\n":
                self._at_line_start = True

        output = "".join(result)
        self._wrapped.write(output)
        return len(s)

    def flush(self) -> None:
        self._wrapped.flush()

    def fileno(self) -> int:
        return int(self._wrapped.fileno())

    @property
    def encoding(self) -> str:
        return getattr(self._wrapped, "encoding", "utf-8")

    def isatty(self) -> bool:
        return bool(self._wrapped.isatty())


@dataclass
class OutputManager:
    """
    Centralized output formatting for recompose.

    Handles all output with consistent tree-style formatting, colors,
    and GHA compatibility.

    Key design: Subprocess output is captured by the parent and prefixed
    at the parent's level. This composes naturally for arbitrary nesting.
    """

    console: Console = field(default_factory=Console)
    verbosity: Verbosity = Verbosity.NORMAL
    _scope_stack: list[ScopeInfo] = field(default_factory=list)
    _is_gha: bool = field(default_factory=lambda: os.environ.get("GITHUB_ACTIONS") == "true")
    _original_stdout: TextIO | None = None
    _original_stderr: TextIO | None = None
    _prefix_writer_installed: bool = False

    @property
    def depth(self) -> int:
        """Current nesting depth."""
        return len(self._scope_stack)

    @property
    def in_gha(self) -> bool:
        """Whether running in GitHub Actions."""
        return self._is_gha

    def push_scope(self, name: str, kind: str = "job") -> ScopeInfo:
        """
        Manually push a scope for depth tracking.

        Use this when you need to track depth without using a context manager.
        Remember to call pop_scope() when done.
        """
        scope = ScopeInfo(name=name, kind=kind, start_time=time.perf_counter())
        self._scope_stack.append(scope)
        return scope

    def pop_scope(self) -> ScopeInfo | None:
        """Pop and return the current scope."""
        if self._scope_stack:
            return self._scope_stack.pop()
        return None

    def _get_line_prefix(self) -> str:
        """Get the prefix for the current line based on scope stack."""
        if not self._scope_stack:
            return ""

        # Build prefix for all levels of depth
        parts = []
        for _ in range(len(self._scope_stack)):
            parts.append(f"{SYMBOLS['pipe']}    ")

        return "".join(parts)

    def _install_prefix_writer(self) -> None:
        """Install the prefix writer on stdout/stderr."""
        if self._prefix_writer_installed or self._is_gha:
            return

        self._original_stdout = sys.stdout
        self._original_stderr = sys.stderr
        sys.stdout = PrefixWriter(self._original_stdout, self._get_line_prefix)
        sys.stderr = PrefixWriter(self._original_stderr, self._get_line_prefix)
        self._prefix_writer_installed = True

    def _uninstall_prefix_writer(self) -> None:
        """Restore original stdout/stderr."""
        if not self._prefix_writer_installed:
            return

        if self._original_stdout is not None:
            sys.stdout = self._original_stdout
        if self._original_stderr is not None:
            sys.stderr = self._original_stderr
        self._prefix_writer_installed = False
        self._original_stdout = None
        self._original_stderr = None

    def _print_raw(self, message: str, style: str | None = None, end: str = "\n") -> None:
        """Print without prefix (used for headers/status)."""
        # If prefix writer is installed, we need to write to original stdout
        # to avoid getting the prefix added
        if self._prefix_writer_installed and self._original_stdout:
            if style:
                # Create a temporary console that writes to original stdout
                # Disable markup and highlighting to prevent Rich from parsing content
                temp_console = Console(file=self._original_stdout, force_terminal=True, highlight=False)
                temp_console.print(message, style=style, end=end, markup=False, highlight=False)
            else:
                self._original_stdout.write(message + end)
                self._original_stdout.flush()
        else:
            if style:
                self.console.print(message, style=style, end=end, markup=False, highlight=False)
            else:
                print(message, end=end, flush=True)

    def task_header(self, name: str, is_nested: bool = False, is_last: bool = False) -> None:
        """Print task header."""
        if self._is_gha:
            print(f"::group::{name}", flush=True)
            return

        if not is_nested:
            # Top-level task
            self._print_raw(f"\n{SYMBOLS['entry']} {name}", style="bold")
            self._print_raw(SYMBOLS["pipe"])
        else:
            # Nested task
            indent = self._get_indent_for_header()
            symbol = SYMBOLS["last"] if is_last else SYMBOLS["branch"]
            self._print_raw(f"{indent}{symbol} {name}", style="bold cyan")

    def task_status(self, name: str, success: bool, elapsed: float, is_nested: bool = False) -> None:
        """Print task completion status."""
        if self._is_gha:
            symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
            status = "succeeded" if success else "failed"
            print(f"{symbol} {name} {status} in {elapsed:.2f}s", flush=True)
            print("::endgroup::", flush=True)
            return

        if not is_nested:
            # Top-level task
            if success:
                self._print_raw(f"\n{SYMBOLS['success']} {name} succeeded in {elapsed:.2f}s", style="bold green")
            else:
                self._print_raw(f"\n{SYMBOLS['failure']} {name} failed in {elapsed:.2f}s", style="bold red")
        else:
            # Nested task - status line at same indent as task content
            # Print indent without styling, then status with styling
            indent = self._get_line_prefix()
            self._print_raw(indent, end="")
            if success:
                self._print_raw(f"{SYMBOLS['success']} {elapsed:.2f}s", style="green")
            else:
                self._print_raw(f"{SYMBOLS['failure']} {elapsed:.2f}s", style="red")

    def _get_indent_for_header(self) -> str:
        """Get indentation for a header line."""
        total_depth = self.depth
        if total_depth == 0:
            return ""

        # Build prefix for all levels of depth
        parts = []
        for _ in range(total_depth):
            parts.append(f"{SYMBOLS['pipe']}    ")

        return "".join(parts)

    def parallel_header(self, job_names: list[str]) -> None:
        """Print header for parallel execution group."""
        if self._is_gha:
            return

        indent = self._get_indent_for_header()
        names_str = ", ".join(job_names)
        self._print_raw(f"{indent}{SYMBOLS['parallel_start']} Running in parallel: {names_str}", style="bold cyan")

    def job_header(self, name: str, is_parallel: bool = False, is_last: bool = False) -> None:
        """Print job header for automation execution."""
        if self._is_gha:
            print(f"::group::{name}", flush=True)
            return

        indent = self._get_indent_for_header()
        if is_parallel:
            symbol = SYMBOLS["parallel_last"] if is_last else SYMBOLS["parallel_branch"]
        else:
            symbol = SYMBOLS["last"] if is_last else SYMBOLS["branch"]

        self._print_raw(f"{indent}{symbol} {name}", style="bold cyan")

    def job_status(self, name: str, success: bool, elapsed: float) -> None:
        """Print job completion status."""
        if self._is_gha:
            symbol = SYMBOLS["success"] if success else SYMBOLS["failure"]
            print(f"{symbol} {name} completed in {elapsed:.2f}s", flush=True)
            print("::endgroup::", flush=True)
            return

        # Print indent without styling, then status with styling
        indent = self._get_line_prefix()
        self._print_raw(indent, end="")
        if success:
            self._print_raw(f"{SYMBOLS['success']} {elapsed:.2f}s", style="green")
        else:
            self._print_raw(f"{SYMBOLS['failure']} {elapsed:.2f}s", style="red")

    def automation_header(self, name: str) -> None:
        """Print automation header."""
        if self._is_gha:
            return

        self._print_raw(f"\n{SYMBOLS['entry']} {name}", style="bold blue")
        self._print_raw(SYMBOLS["pipe"])

    def automation_status(self, name: str, success: bool, elapsed: float, job_count: int) -> None:
        """Print automation completion status."""
        if self._is_gha:
            return

        if success:
            self._print_raw(
                f"\n{SYMBOLS['success']} {name} completed in {elapsed:.2f}s ({job_count} jobs)", style="bold green"
            )
        else:
            self._print_raw(f"\n{SYMBOLS['failure']} {name} failed in {elapsed:.2f}s", style="bold red")

    def step_header(self, name: str) -> None:
        """Print step header within a task."""
        if self._is_gha:
            print(f"::group::{name}", flush=True)
            return

        indent = self._get_line_prefix()
        self._print_raw(f"{indent}[{name}]", style="dim")

    def step_end(self) -> None:
        """Print step end marker."""
        if self._is_gha:
            print("::endgroup::", flush=True)

    def line(self, message: str, style: str | None = None) -> None:
        """Print a line of output with current prefix."""
        if style:
            self.console.print(message, style=style)
        else:
            print(message, flush=True)

    def error(self, message: str) -> None:
        """Print an error message."""
        if self._is_gha:
            print(f"::error::{message}", flush=True)
        else:
            self._print_raw(f"Error: {message}", style="bold red")

    def error_detail(self, lines: list[str], max_lines: int = 5) -> None:
        """Print error detail lines."""
        indent = self._get_line_prefix()
        for line in lines[:max_lines]:
            self._print_raw(f"{indent}  {line}", style="red")

    @contextmanager
    def nested_task_scope(self, name: str, is_last: bool = False) -> Generator[ScopeInfo, None, None]:
        """
        Context manager for nested task output scope.

        Unlike task_scope, this:
        - Always treats the task as nested (uses branch symbols)
        - Does NOT automatically print status (caller must handle result)
        - Yields the ScopeInfo for timing information

        Usage:
            with output_mgr.nested_task_scope("subtask") as scope:
                result = do_work()
            output_mgr.task_status("subtask", result.ok, scope.elapsed, is_nested=True)
        """
        self.task_header(name, is_nested=True, is_last=is_last)

        scope = ScopeInfo(name=name, kind="task", start_time=time.perf_counter(), is_last=is_last)
        self._scope_stack.append(scope)

        if not self._is_gha:
            self._install_prefix_writer()

        try:
            yield scope
        finally:
            self._scope_stack.pop()

            if self.depth == 0:
                self._uninstall_prefix_writer()

    @contextmanager
    def task_scope(self, name: str, is_last: bool = False) -> Generator[None, None, None]:
        """Context manager for top-level task output scope."""
        self.task_header(name, is_nested=False, is_last=is_last)

        scope = ScopeInfo(name=name, kind="task", start_time=time.perf_counter(), is_last=is_last)
        self._scope_stack.append(scope)

        success = True
        try:
            yield
        except Exception:
            success = False
            raise
        finally:
            self._scope_stack.pop()
            elapsed = time.perf_counter() - scope.start_time
            self.task_status(name, success, elapsed, is_nested=False)

    @contextmanager
    def job_scope(self, name: str, is_parallel: bool = False, is_last: bool = False) -> Generator[None, None, None]:
        """Context manager for job output scope (automation execution)."""
        self.job_header(name, is_parallel=is_parallel, is_last=is_last)

        scope = ScopeInfo(name=name, kind="job", start_time=time.perf_counter(), is_last=is_last)
        self._scope_stack.append(scope)

        success = True
        try:
            yield
        except Exception:
            success = False
            raise
        finally:
            self._scope_stack.pop()
            elapsed = time.perf_counter() - scope.start_time
            self.job_status(name, success, elapsed)

    @contextmanager
    def step_scope(self, name: str) -> Generator[None, None, None]:
        """Context manager for step output scope."""
        self.step_header(name)

        scope = ScopeInfo(name=name, kind="step", start_time=time.perf_counter())
        self._scope_stack.append(scope)

        try:
            yield
        finally:
            self._scope_stack.pop()
            self.step_end()

    @contextmanager
    def parallel_scope(self, job_names: list[str]) -> Generator[None, None, None]:
        """Context manager for parallel job group."""
        self.parallel_header(job_names)

        scope = ScopeInfo(name="parallel", kind="parallel", start_time=time.perf_counter())
        self._scope_stack.append(scope)

        try:
            yield
        finally:
            self._scope_stack.pop()

    @contextmanager
    def buffered_output(self) -> Generator[list[str], None, None]:
        """
        Context manager that captures output for later display.

        Used for parallel job execution where output needs to be buffered.
        """
        import io

        buffer = io.StringIO()
        old_stdout = sys.stdout
        old_stderr = sys.stderr
        captured_lines: list[str] = []

        class CapturingWriter:
            def __init__(self, buf: io.StringIO, lines: list[str]):
                self._buffer = buf
                self._lines = lines
                self._current_line = ""

            def write(self, s: str) -> int:
                self._buffer.write(s)
                self._current_line += s
                while "\n" in self._current_line:
                    line, self._current_line = self._current_line.split("\n", 1)
                    self._lines.append(line)
                return len(s)

            def flush(self) -> None:
                self._buffer.flush()
                if self._current_line:
                    self._lines.append(self._current_line)
                    self._current_line = ""

            def fileno(self) -> int:
                return 1

            @property
            def encoding(self) -> str:
                return "utf-8"

            def isatty(self) -> bool:
                return False

        sys.stdout = CapturingWriter(buffer, captured_lines)
        sys.stderr = CapturingWriter(buffer, captured_lines)

        try:
            yield captured_lines
        finally:
            sys.stdout = old_stdout
            sys.stderr = old_stderr

    def print_buffered_output(self, lines: list[str], prefix: str = "") -> None:
        """Print buffered output lines with optional prefix."""
        for line in lines:
            if prefix:
                self._print_raw(f"{prefix}{line}")
            else:
                self._print_raw(line)

    def print_job_output(self, lines: list[str], verbose: bool = False) -> None:
        """Print captured job output lines."""
        if not lines:
            return

        indent = self._get_line_prefix()
        for line in lines:
            self._print_raw(f"{indent}{line}")


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
    if _output_manager is not None:
        _output_manager._uninstall_prefix_writer()
    _output_manager = None


def is_tree_mode() -> bool:
    """
    Check if tree output mode is enabled.

    This is a compatibility function for the subprocess module.
    Returns True if we're inside a nested output scope.
    """
    mgr = get_output_manager()
    return mgr.depth > 0


def configure_output(
    verbosity: Verbosity = Verbosity.NORMAL,
    force_color: bool | None = None,
) -> OutputManager:
    """
    Configure the global output manager.

    Args:
        verbosity: Output verbosity level
        force_color: Force color output on/off (None for auto-detect)

    Returns:
        The configured OutputManager instance.

    """
    global _output_manager

    console_kwargs: dict[str, Any] = {}
    if force_color is not None:
        console_kwargs["force_terminal"] = force_color

    _output_manager = OutputManager(
        console=Console(**console_kwargs),
        verbosity=verbosity,
    )
    return _output_manager
