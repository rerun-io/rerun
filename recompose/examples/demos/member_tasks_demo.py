#!/usr/bin/env python3
"""
Example demonstrating class-based member tasks.

Run with:
    cd recompose
    uv run python examples/member_tasks_demo.py --help
    uv run python examples/member_tasks_demo.py counter.increment --start=10 --by=5
    uv run python examples/member_tasks_demo.py counter.show --start=42
    uv run python examples/member_tasks_demo.py fileops.list --directory=/tmp
"""

from pathlib import Path

import recompose


@recompose.taskclass
class Counter:
    """A simple counter demonstrating class-based tasks."""

    def __init__(self, *, start: int = 0):
        """Initialize the counter with a starting value."""
        self.value = start
        recompose.dbg(f"Counter initialized with value={self.value}")

    @recompose.task
    def increment(self, *, by: int = 1) -> recompose.Result[int]:
        """Increment the counter by a given amount."""
        self.value += by
        recompose.out(f"Incremented {self.value - by} by {by} = {self.value}")
        return recompose.Ok(self.value)

    @recompose.task
    def show(self) -> recompose.Result[int]:
        """Show the current counter value."""
        recompose.out(f"Counter value: {self.value}")
        return recompose.Ok(self.value)


@recompose.taskclass
class FileOps:
    """File operations demonstrating member tasks with subprocess."""

    def __init__(self, *, directory: str = "."):
        """Initialize with a target directory."""
        self.directory = Path(directory)
        recompose.dbg(f"FileOps initialized for directory: {self.directory}")

    @recompose.task
    def list(self, *, long: bool = False) -> recompose.Result[int]:
        """List files in the directory."""
        recompose.out(f"Listing files in {self.directory}")

        args = ["ls"]
        if long:
            args.append("-la")
        args.append(str(self.directory))

        result = recompose.run(*args)
        return recompose.Ok(result.returncode)

    @recompose.task
    def count(self) -> recompose.Result[int]:
        """Count files in the directory."""
        if not self.directory.exists():
            return recompose.Err(f"Directory does not exist: {self.directory}")

        files = list(self.directory.iterdir())
        recompose.out(f"Found {len(files)} items in {self.directory}")
        return recompose.Ok(len(files))


# You can also have standalone tasks alongside class tasks
@recompose.task
def greet(*, name: str = "World") -> recompose.Result[str]:
    """A simple greeting task."""
    message = f"Hello, {name}!"
    recompose.out(message)
    return recompose.Ok(message)


if __name__ == "__main__":
    recompose.main()
