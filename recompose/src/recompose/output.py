"""Tree-based output rendering for flow execution.

This module provides utilities for rendering flow execution in a tree format.
Steps receive context via environment variables and render themselves appropriately.

Environment variables:
    RECOMPOSE_TREE_MODE: Set to "1" to enable tree output mode
    RECOMPOSE_TREE_PREFIX: The prefix to use for output lines (e.g., "│   ")
    RECOMPOSE_STEP_INDEX: Current step index (1-based)
    RECOMPOSE_TOTAL_STEPS: Total number of steps in the flow

Example output:

    my_flow
    │
    ├─▶ 1_setup                           ✓ 0.00s
    │     Running setup...
    │     → setup-complete
    │
    ├─▶ 2_eval_condition                  ✓ 0.00s
    │     → False
    │
    ├─▶ 3_extra_validation                ⏭ skipped
    │     ╰─ reason: condition false
    │
    └─▶ 4_finalize                        ✓ 0.01s
          Finalizing...
          → done

    ⏹ SUCCESS  my_flow  (0.25s)

"""

from __future__ import annotations

import io
import logging
import os
import sys
from typing import TYPE_CHECKING, TextIO

if TYPE_CHECKING:
    from rich.console import Console

# Tree drawing characters
TREE_BRANCH = "├─▶"
TREE_BRANCH_LAST = "└─▶"
TREE_CONT = "│    "
TREE_CONT_LAST = "     "
TREE_REASON = "╰─"

# Environment variable names
ENV_TREE_MODE = "RECOMPOSE_TREE_MODE"
ENV_TREE_PREFIX = "RECOMPOSE_TREE_PREFIX"
ENV_STEP_INDEX = "RECOMPOSE_STEP_INDEX"
ENV_TOTAL_STEPS = "RECOMPOSE_TOTAL_STEPS"
ENV_FLOW_NAME = "RECOMPOSE_FLOW_NAME"


def is_tree_mode() -> bool:
    """Check if tree output mode is enabled."""
    return os.environ.get(ENV_TREE_MODE) == "1"


def get_tree_prefix() -> str:
    """Get the current tree prefix for output lines."""
    return os.environ.get(ENV_TREE_PREFIX, "")


def get_step_context() -> tuple[int, int] | None:
    """
    Get the current step context.

    Returns:
        Tuple of (step_index, total_steps) or None if not in a step.

    """
    step_idx = os.environ.get(ENV_STEP_INDEX)
    total = os.environ.get(ENV_TOTAL_STEPS)
    if step_idx and total:
        return int(step_idx), int(total)
    return None


class TreePrefixWriter(io.TextIOBase):
    """
    A TextIO wrapper that prefixes each line with the tree continuation character.

    This is used to wrap sys.stdout and sys.stderr during step execution so that
    all Python output (print, logging, etc.) is properly formatted with tree prefixes.
    """

    def __init__(self, wrapped: TextIO, prefix: str, is_stderr: bool = False):
        """
        Initialize the wrapper.

        Args:
            wrapped: The original TextIO to wrap (e.g., sys.stdout)
            prefix: The tree prefix to add (e.g., "│    ")
            is_stderr: If True, use error indicator styling

        """
        self._wrapped = wrapped
        self._prefix = prefix
        self._is_stderr = is_stderr
        self._at_line_start = True

    def write(self, s: str) -> int:
        """Write string with tree prefix at the start of each line."""
        if not s:
            return 0

        result = []
        for char in s:
            if self._at_line_start and char != "\n":
                # Add prefix at the start of a new line
                result.append(self._prefix)
                result.append(" ")
                self._at_line_start = False
            result.append(char)
            if char == "\n":
                self._at_line_start = True

        output = "".join(result)
        self._wrapped.write(output)
        return len(s)

    def flush(self) -> None:
        """Flush the wrapped stream."""
        self._wrapped.flush()

    def fileno(self) -> int:
        """Return the file descriptor of the wrapped stream."""
        return self._wrapped.fileno()

    @property
    def encoding(self) -> str:  # type: ignore[override]
        """Return the encoding of the wrapped stream."""
        return getattr(self._wrapped, "encoding", "utf-8")

    def isatty(self) -> bool:
        """Return whether the wrapped stream is a TTY."""
        return self._wrapped.isatty()


class TreeOutputContext:
    """
    Context manager that wraps stdout/stderr with tree-prefixed writers.

    Usage:
        with TreeOutputContext():
            print("This will be prefixed")
            logging.info("This too")

    """

    def __init__(self) -> None:
        self._original_stdout: TextIO | None = None
        self._original_stderr: TextIO | None = None
        self._original_handlers: list[tuple[logging.StreamHandler[TextIO], TextIO]] = []

    def __enter__(self) -> TreeOutputContext:
        if is_tree_mode():
            prefix = get_tree_prefix()
            self._original_stdout = sys.stdout
            self._original_stderr = sys.stderr
            sys.stdout = TreePrefixWriter(self._original_stdout, prefix, is_stderr=False)
            sys.stderr = TreePrefixWriter(self._original_stderr, prefix, is_stderr=True)

            # Update logging handlers to use wrapped streams
            for handler in logging.root.handlers:
                if isinstance(handler, logging.StreamHandler):
                    if handler.stream is self._original_stdout:
                        self._original_handlers.append((handler, handler.stream))
                        handler.stream = sys.stdout
                    elif handler.stream is self._original_stderr:
                        self._original_handlers.append((handler, handler.stream))
                        handler.stream = sys.stderr
        return self

    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> None:
        # Restore logging handlers first
        for handler, original_stream in self._original_handlers:
            handler.stream = original_stream
        self._original_handlers.clear()

        # Restore stdout/stderr
        if self._original_stdout is not None:
            sys.stdout = self._original_stdout
        if self._original_stderr is not None:
            sys.stderr = self._original_stderr


def install_tree_output() -> TreeOutputContext | None:
    """
    Install tree-prefixed stdout/stderr if in tree mode.

    Returns:
        The context manager if installed, or None if not in tree mode.
        Call .close() or use as context manager to restore original streams.

    """
    if is_tree_mode():
        ctx = TreeOutputContext()
        ctx.__enter__()
        return ctx
    return None


def uninstall_tree_output(ctx: TreeOutputContext | None) -> None:
    """Restore original stdout/stderr."""
    if ctx is not None:
        ctx.__exit__(None, None, None)


class FlowRenderer:
    """
    Render flow execution in tree format.

    The renderer handles the tree structure (branches, headers, footers).
    Individual steps render their own output using the tree prefix from
    environment variables.

    """

    def __init__(self, console: Console, flow_name: str, total_steps: int):
        """
        Initialize the renderer.

        Args:
            console: Rich console for output
            flow_name: Name of the flow being executed
            total_steps: Total number of steps in the flow

        """
        self.console = console
        self.flow_name = flow_name
        self.total_steps = total_steps
        self._step_index = 0

    def start(self) -> None:
        """Print the flow header."""
        self.console.print()
        self.console.print(f"[bold]{self.flow_name}[/bold]")
        self.console.print("│")

    def get_step_env(self, step_index: int) -> dict[str, str]:
        """
        Get environment variables to pass to a step subprocess.

        Args:
            step_index: 1-based index of the step

        Returns:
            Dict of environment variables to set

        """
        # Always use continuation prefix - line continues to final ⏹
        return {
            ENV_TREE_MODE: "1",
            ENV_TREE_PREFIX: TREE_CONT,
            ENV_STEP_INDEX: str(step_index),
            ENV_TOTAL_STEPS: str(self.total_steps),
            ENV_FLOW_NAME: self.flow_name,
        }

    def step_header(self, step_name: str, step_index: int) -> None:
        """
        Print the step header line.

        Args:
            step_name: Name of the step
            step_index: 1-based index of the step

        """
        # Always use branch (not last branch) - line continues to final ⏹
        self.console.print(f"{TREE_BRANCH} [bold]{step_name}[/bold]")

    def step_success(self, step_name: str, step_index: int, duration: float, value: object = None) -> None:
        """
        Print the step success footer with result and timing.

        Args:
            step_name: Name of the step
            step_index: 1-based index of the step
            duration: Step duration in seconds
            value: Optional result value to display

        """
        # Always use continuation prefix - line continues to final ⏹
        if value is not None:
            self.console.print(f"{TREE_CONT} [dim]→[/dim] {value}")
        self.console.print(f"{TREE_CONT} [bold green]✓[/bold green] [dim]succeeded in {duration:.2f}s[/dim]")
        self.console.print("│")

    def step_failed(self, step_name: str, step_index: int, duration: float, error: str | None = None) -> None:
        """
        Print the step failure footer with error and timing.

        Args:
            step_name: Name of the step
            step_index: 1-based index of the step
            duration: Step duration in seconds
            error: Optional error message

        """
        # Always use continuation prefix - line continues to final ⏹
        if error:
            self.console.print(f"{TREE_CONT} [red]error: {error}[/red]")
        self.console.print(f"{TREE_CONT} [bold red]✗[/bold red] [dim]failed in {duration:.2f}s[/dim]")
        self.console.print("│")

    def step_skipped(self, step_name: str, step_index: int, reason: str) -> None:
        """
        Print a skipped step.

        Args:
            step_name: Name of the step
            step_index: 1-based index of the step
            reason: Reason the step was skipped

        """
        # Always use branch/continuation - line continues to final ⏹
        self.console.print(f"{TREE_BRANCH} [dim]{step_name}[/dim]")
        self.console.print(f"{TREE_CONT} [dim]⏭ skipped: {reason}[/dim]")
        self.console.print("│")

    def step_condition(
        self, step_name: str, step_index: int, condition_expr: str, value: bool, duration: float
    ) -> None:
        """
        Print a condition evaluation step with result.

        Args:
            step_name: Name of the step (e.g., "2_eval_condition")
            step_index: 1-based index of the step
            condition_expr: String representation of the condition being evaluated
            value: The condition result (True/False)
            duration: Evaluation duration in seconds

        """
        # Always use branch/continuation - line continues to final ⏹
        # Print header with condition expression
        self.console.print(f"{TREE_BRANCH} [cyan]{step_name}[/cyan]  [dim]({condition_expr})[/dim]")

        # Print result value
        value_style = "green" if value else "yellow"
        self.console.print(f"{TREE_CONT} [bold {value_style}]→ {value}[/bold {value_style}]")

        # Print timing
        self.console.print(f"{TREE_CONT} [bold green]✓[/bold green] [dim]succeeded in {duration:.2f}s[/dim]")
        self.console.print("│")

    def finish(self, success: bool, duration: float) -> None:
        """
        Print the flow completion summary.

        The ⏹ symbol terminates the tree line.

        Args:
            success: Whether the flow succeeded
            duration: Total flow duration in seconds

        """
        if success:
            self.console.print(f"[bold green]⏹[/bold green] Completed in {duration:.2f}s")
        else:
            self.console.print(f"[bold red]⏹[/bold red] Failed in {duration:.2f}s")
        self.console.print()
