"""Command group and App for explicit CLI registration."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .task import TaskWrapper


@dataclass
class CommandGroup:
    """
    Groups commands under a heading in help output.

    Commands remain in a flat namespace - groups only affect the visual
    organization in `--help` output.

    Args:
        name: Heading name displayed in help (e.g., "Python", "Testing").
        commands: List of tasks to include in this group.
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
    commands: list[TaskWrapper[Any, Any]] = field(default_factory=list)
    hidden: bool = False

    def __post_init__(self) -> None:
        """Validate that commands list is not empty."""
        if not self.commands:
            raise ValueError(f"CommandGroup '{self.name}' must have at least one command")


class App:
    """
    Recompose application that holds configuration and command registration.

    Create an App instance at module level (outside of `if __name__ == "__main__"`)
    so that subprocess invocations can import the module and access the app's
    configuration and registered commands.

    Args:
        python_cmd: Command to invoke Python in generated GHA workflows.
                   Use "uv run python" for uv-managed projects.
        working_directory: Working directory for GHA workflows (relative to repo root).
                          If set, workflows will cd to this directory before running.
        commands: List of CommandGroups or tasks to expose as CLI commands.
        automations: List of automations to register for GHA workflow generation.
        dispatchables: List of dispatchables to register for GHA workflow generation.
        name: Optional name for the CLI group. Defaults to the script name.

    Example
    -------
        # examples/app.py
        import recompose
        from .tasks import lint, test
        from .automations import ci

        lint_workflow = recompose.make_dispatchable(lint)

        app = recompose.App(
            python_cmd="uv run python",
            working_directory="recompose",
            commands=[
                recompose.CommandGroup("Quality", [lint]),
                recompose.CommandGroup("Testing", [test]),
            ],
            automations=[ci],
            dispatchables=[lint_workflow],
        )

        if __name__ == "__main__":
            app.main()

    """

    def __init__(
        self,
        *,
        python_cmd: str = "python",
        working_directory: str | None = None,
        commands: Sequence[CommandGroup | TaskWrapper[Any, Any]] | None = None,
        automations: Sequence[Any] | None = None,
        dispatchables: Sequence[Any] | None = None,
        name: str | None = None,
    ) -> None:
        """
        Initialize the recompose application.

        Args:
            python_cmd: Command to invoke Python in generated GHA workflows.
            working_directory: Working directory for GHA workflows (relative to repo root).
            commands: List of CommandGroups or tasks to expose as CLI commands.
            automations: List of automations to register for GHA workflow generation.
            dispatchables: List of dispatchables to register for GHA workflow generation.
            name: Optional name for the CLI group. Defaults to the script name.

        """
        import sys

        self.python_cmd = python_cmd
        self.working_directory = working_directory
        self.commands: Sequence[CommandGroup | TaskWrapper[Any, Any]] = commands or []
        self.automations: Sequence[Any] = automations or []
        self.dispatchables: Sequence[Any] = dispatchables or []
        self.name = name

        # Capture the caller's module name at instantiation time
        # This is required for subprocess isolation - the module must be importable
        caller_frame = sys._getframe(1)
        caller_spec = caller_frame.f_globals.get("__spec__")
        if caller_spec is not None and caller_spec.name:
            self._module_name: str = caller_spec.name
        else:
            raise ValueError(
                "App must be instantiated in a module context (run with `python -m <module>`). "
                "Script-based execution is not supported because GHA workflow generation "
                "needs an importable module path."
            )

    def main(self) -> None:
        """
        Build and run the CLI.

        This should be called inside `if __name__ == "__main__":` to avoid
        running the CLI when the module is imported.
        """
        from .cli import main as cli_main

        cli_main(
            name=self.name,
            python_cmd=self.python_cmd,
            working_directory=self.working_directory,
            commands=self.commands,
            automations=self.automations,
            dispatchables=self.dispatchables,
            module_name=self._module_name,
        )

    def setup_context(self) -> None:
        """
        Set up the global context from this app's configuration.

        This ensures that tasks like generate_gha have access to the correct
        configuration (working_directory, python_cmd, etc.) even when not
        running through main().
        """
        from .cli import _build_registry
        from .context import (
            set_module_name,
            set_python_cmd,
            set_recompose_context,
            set_working_directory,
        )

        # Set config values
        set_python_cmd(self.python_cmd)
        set_working_directory(self.working_directory)

        # Set module name (for GHA workflow generation)
        set_module_name(self._module_name)

        # Build and set the registry
        recompose_ctx = _build_registry(self.commands, self.automations or [], self.dispatchables or [])
        set_recompose_context(recompose_ctx)
