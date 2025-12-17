#!/usr/bin/env python3
"""
Tutorial: Task Classes

This tutorial introduces @taskclass for stateful, grouped tasks:
- The @taskclass decorator turns a class into a task group
- Constructor parameters become shared CLI options
- Member methods decorated with @task become sub-commands
- Useful for tasks that share configuration or state

Run this file to see all tasks:
    uv run python examples/tutorial/intro_taskclass.py --help

Try the Counter taskclass:
    uv run python examples/tutorial/intro_taskclass.py counter --help
    uv run python examples/tutorial/intro_taskclass.py counter.increment --start=10
    uv run python examples/tutorial/intro_taskclass.py counter.increment --start=10 --by=5
    uv run python examples/tutorial/intro_taskclass.py counter.show --start=42

Try the FileOps taskclass:
    uv run python examples/tutorial/intro_taskclass.py fileops.list --directory=/tmp
    uv run python examples/tutorial/intro_taskclass.py fileops.count --directory=/tmp
"""

from pathlib import Path

import recompose

# =============================================================================
# BASIC TASKCLASS
# =============================================================================
#
# @taskclass turns a class into a group of related tasks.
# Constructor args become shared CLI options for all member tasks.


@recompose.taskclass
class Counter:
    """
    A simple counter demonstrating @taskclass.

    The constructor's `start` parameter becomes a shared CLI option.
    Member methods decorated with @task become sub-commands.

    CLI usage:
        counter.increment --start=10 --by=5
        counter.show --start=42
    """

    def __init__(self, *, start: int = 0):
        """
        Initialize the counter.

        Args:
            start: Initial counter value (becomes --start CLI option)

        """
        self.value = start
        recompose.dbg(f"Counter initialized with value={self.value}")

    @recompose.task
    def increment(self, *, by: int = 1) -> recompose.Result[int]:
        """
        Increment the counter.

        Args:
            by: Amount to increment (becomes --by CLI option)

        """
        old_value = self.value
        self.value += by
        recompose.out(f"Incremented {old_value} by {by} = {self.value}")
        return recompose.Ok(self.value)

    @recompose.task
    def decrement(self, *, by: int = 1) -> recompose.Result[int]:
        """Decrement the counter."""
        old_value = self.value
        self.value -= by
        recompose.out(f"Decremented {old_value} by {by} = {self.value}")
        return recompose.Ok(self.value)

    @recompose.task
    def show(self) -> recompose.Result[int]:
        """Show the current counter value."""
        recompose.out(f"Counter value: {self.value}")
        return recompose.Ok(self.value)


# =============================================================================
# PRACTICAL TASKCLASS
# =============================================================================
#
# Task classes are great for grouping related operations that share context.


@recompose.taskclass
class FileOps:
    """
    File operations on a directory.

    Demonstrates a practical use of @taskclass where multiple
    operations share a common directory configuration.
    """

    def __init__(self, *, directory: str = "."):
        """
        Initialize with target directory.

        Args:
            directory: Directory to operate on (becomes --directory CLI option)

        """
        self.directory = Path(directory)
        recompose.dbg(f"FileOps initialized for: {self.directory}")

    @recompose.task
    def list(self, *, long: bool = False) -> recompose.Result[int]:
        """
        List files in the directory.

        Args:
            long: Use long format (becomes --long flag)

        """
        recompose.out(f"Listing files in {self.directory}")

        args = ["ls"]
        if long:
            args.append("-la")
        args.append(str(self.directory))

        result = recompose.run(*args)
        return recompose.Ok(result.returncode)

    @recompose.task
    def count(self) -> recompose.Result[int]:
        """Count items in the directory."""
        if not self.directory.exists():
            return recompose.Err(f"Directory does not exist: {self.directory}")

        items = list(self.directory.iterdir())
        recompose.out(f"Found {len(items)} items in {self.directory}")
        return recompose.Ok(len(items))

    @recompose.task
    def size(self) -> recompose.Result[int]:
        """Get total size of files in the directory."""
        if not self.directory.exists():
            return recompose.Err(f"Directory does not exist: {self.directory}")

        total = 0
        for item in self.directory.iterdir():
            if item.is_file():
                total += item.stat().st_size

        recompose.out(f"Total size: {total:,} bytes")
        return recompose.Ok(total)


# =============================================================================
# ENTRYPOINT
# =============================================================================

if __name__ == "__main__":
    # Access task wrappers via _recompose_tasks on the class
    commands = [
        recompose.CommandGroup("Counter", list(Counter._recompose_tasks.values())),  # type: ignore[attr-defined]
        recompose.CommandGroup("FileOps", list(FileOps._recompose_tasks.values())),  # type: ignore[attr-defined]
    ]
    recompose.main(commands=commands)
