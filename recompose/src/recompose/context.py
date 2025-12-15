"""Execution context for recompose tasks."""

from __future__ import annotations

from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Literal

from rich.console import Console

# Global console for output
_console = Console()

# Debug mode flag
_debug_mode: bool = False

# Entry point info (set by main())
# Tuple of (type, value) where type is "module" or "script"
_entry_point: tuple[str, str] | None = None

# Python command for GHA workflow generation (e.g., "python", "uv run python")
_python_cmd: str = "python"

# Working directory for GHA workflow generation (relative to repo root)
_working_directory: str | None = None


@dataclass
class OutputLine:
    """A captured line of output."""

    level: Literal["out", "dbg"]
    message: str


@dataclass
class Context:
    """
    Execution context for a task.

    Tracks output and provides task metadata.
    """

    task_name: str
    output: list[OutputLine] = field(default_factory=list)

    def capture_out(self, message: str) -> None:
        """Capture an output line."""
        self.output.append(OutputLine(level="out", message=message))

    def capture_dbg(self, message: str) -> None:
        """Capture a debug line."""
        self.output.append(OutputLine(level="dbg", message=message))


# Context variable for the current task context
_current_context: ContextVar[Context | None] = ContextVar("recompose_context", default=None)


def get_context() -> Context | None:
    """Get the current task context, or None if not in a task."""
    return _current_context.get()


def set_context(ctx: Context | None) -> None:
    """Set the current task context."""
    _current_context.set(ctx)


def set_debug(enabled: bool) -> None:
    """Enable or disable debug output."""
    global _debug_mode
    _debug_mode = enabled


def is_debug() -> bool:
    """Check if debug mode is enabled."""
    return _debug_mode


def set_entry_point(entry_type: str, value: str) -> None:
    """
    Set the entry point info (called by main()).

    Args:
        entry_type: "module" or "script"
        value: Module name (e.g., "examples.app") or script path
    """
    global _entry_point
    _entry_point = (entry_type, value)


def get_entry_point() -> tuple[str, str] | None:
    """
    Get the entry point info.

    Returns:
        Tuple of (type, value) where type is "module" or "script",
        or None if not set.
    """
    return _entry_point


def set_python_cmd(cmd: str) -> None:
    """
    Set the Python command for GHA workflow generation.

    Args:
        cmd: Command to invoke Python (e.g., "python", "uv run python").
    """
    global _python_cmd
    _python_cmd = cmd


def get_python_cmd() -> str:
    """
    Get the Python command for GHA workflow generation.

    Returns:
        Command to invoke Python (default: "python").
    """
    return _python_cmd


def set_working_directory(directory: str | None) -> None:
    """
    Set the working directory for GHA workflow generation.

    Args:
        directory: Working directory relative to repo root, or None for repo root.
    """
    global _working_directory
    _working_directory = directory


def get_working_directory() -> str | None:
    """
    Get the working directory for GHA workflow generation.

    Returns:
        Working directory relative to repo root, or None for repo root.
    """
    return _working_directory


def out(message: str) -> None:
    """
    Output a message.

    When running inside a task context, the message is captured.
    Always prints to console.
    """
    ctx = _current_context.get()
    if ctx is not None:
        ctx.capture_out(message)
    _console.print(message)


def dbg(message: str) -> None:
    """
    Output a debug message.

    When running inside a task context, the message is captured.
    Only prints to console if debug mode is enabled.
    """
    ctx = _current_context.get()
    if ctx is not None:
        ctx.capture_dbg(message)
    if _debug_mode:
        _console.print(f"[dim]{message}[/dim]")
