"""Command group and configuration for explicit CLI registration."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .flow import FlowWrapper
    from .task import TaskWrapper


@dataclass
class Config:
    """
    Configuration for recompose CLI.

    Args:
        python_cmd: Command to invoke Python in generated GHA workflows.
                   Use "uv run python" for uv-managed projects.
        working_directory: Working directory for GHA workflows (relative to repo root).
                          If set, workflows will cd to this directory before running.

    Example
    -------
        config = recompose.Config(
            python_cmd="uv run python",
            working_directory="recompose",
        )
        recompose.main(config=config, commands=[...])

    """

    python_cmd: str = "python"
    working_directory: str | None = None


@dataclass
class CommandGroup:
    """
    Groups commands under a heading in help output.

    Commands remain in a flat namespace - groups only affect the visual
    organization in `--help` output.

    Args:
        name: Heading name displayed in help (e.g., "Python", "Testing").
        commands: List of tasks and/or flows to include in this group.
        hidden: If True, commands in this group are hidden from default help.
               Use --show-hidden to see them.

    Example
    -------
        commands = [
            recompose.CommandGroup("Quality", [lint, format_check, format]),
            recompose.CommandGroup("Testing", [test, test_installed]),
            recompose.CommandGroup("Internal", [debug_task], hidden=True),
        ]

    """

    name: str
    commands: list[TaskWrapper | FlowWrapper] = field(default_factory=list)
    hidden: bool = False

    def __post_init__(self) -> None:
        """Validate that commands list is not empty."""
        if not self.commands:
            raise ValueError(f"CommandGroup '{self.name}' must have at least one command")
