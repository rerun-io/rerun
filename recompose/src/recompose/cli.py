"""CLI generation for recompose tasks."""

from __future__ import annotations

import inspect
import time
from collections.abc import Sequence
from enum import Enum
from pathlib import Path
from typing import Any, cast, get_args, get_origin

import click
from rich.console import Console

from .command_group import CommandGroup
from .context import (
    RecomposeContext,
    set_debug,
    set_module_name,
    set_python_cmd,
    set_recompose_context,
    set_working_directory,
)
from .result import Result
from .task import TaskInfo, TaskWrapper

console = Console()


def _to_kebab_case(name: str) -> str:
    """Convert a snake_case name to kebab-case for CLI."""
    return name.replace("_", "-")


def _get_click_type(annotation: Any) -> tuple[type | click.ParamType, bool]:
    """
    Convert a Python type annotation to a Click type.

    Returns (click_type, is_required).
    """
    # Handle Optional types (Union with None)
    origin = get_origin(annotation)
    if origin is type(None):
        return click.STRING, False

    # Check for Optional (Union[X, None])
    if origin is not None:
        args = get_args(annotation)
        # Handle Optional[X] which is Union[X, None]
        if type(None) in args:
            # Get the non-None type
            non_none_types = [a for a in args if a is not type(None)]
            if len(non_none_types) == 1:
                inner_type, _ = _get_click_type(non_none_types[0])
                return inner_type, False

    # Handle basic types
    if annotation is str:
        return click.STRING, True
    elif annotation is int:
        return click.INT, True
    elif annotation is float:
        return click.FLOAT, True
    elif annotation is bool:
        return click.BOOL, True
    elif annotation is Path or annotation is type(Path):
        return click.Path(), True
    elif isinstance(annotation, type) and issubclass(annotation, Enum):
        # Enum becomes a Choice of its values
        choices = [e.value for e in annotation]
        return click.Choice(choices), True
    else:
        # Default to string
        return click.STRING, True


def _build_command(task_info: TaskInfo) -> click.Command:
    """Build a Click command from a task."""
    sig = task_info.signature
    params: list[click.Parameter] = []

    # Use get_type_hints to resolve string annotations from `from __future__ import annotations`
    import typing

    try:
        type_hints = typing.get_type_hints(task_info.original_fn)
    except Exception:
        type_hints = {}

    for param_name, param in sig.parameters.items():
        if param_name == "self":
            continue

        # Get type annotation - prefer resolved type hints
        annotation = type_hints.get(param_name, param.annotation)
        if annotation is inspect.Parameter.empty:
            annotation = str  # Default to string if no annotation

        click_type, type_required = _get_click_type(annotation)

        # Check if there's a default
        has_default = param.default is not inspect.Parameter.empty
        default_value = param.default if has_default else None

        # Determine if required
        required = not has_default and type_required

        # Handle bool specially (use flag style)
        # Convert underscores to hyphens for CLI option names (kebab-case)
        cli_name = _to_kebab_case(param_name)
        if annotation is bool:
            if has_default and default_value is True:
                params.append(
                    click.Option(
                        [f"--{cli_name}/--no-{cli_name}"],
                        default=True,
                        help="(default: True)",
                    )
                )
            elif has_default and default_value is False:
                params.append(
                    click.Option(
                        [f"--{cli_name}/--no-{cli_name}"],
                        default=False,
                        help="(default: False)",
                    )
                )
            else:
                params.append(
                    click.Option(
                        [f"--{cli_name}/--no-{cli_name}"],
                        default=False,
                        required=required,
                    )
                )
        else:
            help_text = None
            if has_default and default_value is not None:
                help_text = f"(default: {default_value})"

            # Only pass default if there is one - otherwise Click won't enforce required
            option_kwargs: dict[str, Any] = {
                "type": click_type,
                "required": required,
                "help": help_text,
            }
            if has_default:
                option_kwargs["default"] = default_value

            params.append(
                click.Option(
                    [f"--{cli_name}"],
                    **option_kwargs,
                )
            )

    def callback(**kwargs: Any) -> None:
        """Execute the task and display results."""
        task_name = task_info.name

        # Start timing
        start_time = time.perf_counter()

        # Print task header
        console.print(f"\n[bold blue]▶[/bold blue] [bold]{task_name}[/bold]")
        console.print()

        # Convert enum values back to enum if needed
        for param_name, param in sig.parameters.items():
            if param_name in kwargs:
                annotation = param.annotation
                if isinstance(annotation, type) and issubclass(annotation, Enum):
                    # Convert string value back to enum
                    value = kwargs[param_name]
                    if value is not None:
                        kwargs[param_name] = annotation(value)

        # Execute the task
        result: Result[Any] = task_info.fn(**kwargs)

        # End timing
        elapsed = time.perf_counter() - start_time

        # Print result
        console.print()
        if result.ok:
            console.print(f"[bold green]✓[/bold green] [bold]{task_name}[/bold] succeeded in {elapsed:.2f}s")
            if result._value is not None:
                console.print(f"[dim]→[/dim] {result._value}")
        else:
            console.print(f"[bold red]✗[/bold red] [bold]{task_name}[/bold] failed in {elapsed:.2f}s")
            if result.error:
                console.print(f"[red]Error:[/red] {result.error}")
            if result.traceback:
                from .context import is_debug

                if is_debug():
                    console.print(f"[dim]{result.traceback}[/dim]")

        console.print()

    # Build the command with kebab-case name
    cmd = click.Command(
        name=_to_kebab_case(task_info.name),
        callback=callback,
        params=params,
        help=task_info.doc,
    )

    return cmd


class GroupedClickGroup(click.Group):
    """Click group that displays commands organized by groups in help."""

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        self.command_groups: dict[str, str] = {}  # command_name -> group_name
        self.hidden_groups: set[str] = set()
        self.show_hidden: bool = False
        super().__init__(*args, **kwargs)

    def format_commands(self, ctx: click.Context, formatter: click.HelpFormatter) -> None:
        """Format commands grouped by category."""
        # Collect commands by group
        groups: dict[str, list[tuple[str, click.Command]]] = {}
        for name in self.list_commands(ctx):
            cmd = self.get_command(ctx, name)
            if cmd is None:
                continue

            # Skip hidden commands unless --show-hidden
            if cmd.hidden and not self.show_hidden:
                continue

            group_name = self.command_groups.get(name, "Other")

            # Skip hidden groups unless --show-hidden
            if group_name in self.hidden_groups and not self.show_hidden:
                continue

            if group_name not in groups:
                groups[group_name] = []
            groups[group_name].append((name, cmd))

        # Format each group
        for group_name, cmds in groups.items():
            with formatter.section(group_name):
                formatter.write_dl(
                    [(name, cmd.get_short_help_str(limit=45)) for name, cmd in sorted(cmds, key=lambda x: x[0])]
                )


def _build_grouped_cli(
    name: str | None,
    commands: Sequence[CommandGroup | TaskWrapper[Any, Any]],
) -> GroupedClickGroup:
    """Build a Click CLI with grouped commands."""
    # Validate no duplicate command names
    seen_names: dict[str, str] = {}  # name -> group_name

    @click.group(name=name, cls=GroupedClickGroup)
    @click.option("--debug/--no-debug", default=False, help="Enable debug output")
    @click.option("--show-hidden", is_flag=True, default=False, help="Show hidden commands")
    @click.pass_context
    def cli(ctx: click.Context, debug: bool, show_hidden: bool) -> None:
        """Recompose task runner."""
        ctx.ensure_object(dict)
        set_debug(debug)
        # Store show_hidden on the group for format_commands
        ctx.command.show_hidden = show_hidden  # type: ignore[attr-defined]

    # Process commands and groups
    for item in commands:
        if isinstance(item, CommandGroup):
            group_name = item.name
            if item.hidden:
                cli.hidden_groups.add(group_name)

            for cmd_wrapper in item.commands:
                _add_command_to_cli(cli, cmd_wrapper, group_name, seen_names)
        else:
            # Bare task (not in a group)
            _add_command_to_cli(cli, item, "Other", seen_names)

    return cli


def _add_command_to_cli(
    cli: GroupedClickGroup,
    cmd_wrapper: TaskWrapper[Any, Any],
    group_name: str,
    seen_names: dict[str, str],
) -> None:
    """Add a task to the CLI, checking for duplicates."""
    # Get the info object from the wrapper
    if hasattr(cmd_wrapper, "_task_info"):
        info = cast(TaskWrapper[Any, Any], cmd_wrapper)._task_info
    else:
        raise TypeError(f"Expected a task, got {type(cmd_wrapper).__name__}. Make sure to use @task decorator.")

    # Use kebab-case for CLI command names
    cmd_name = _to_kebab_case(info.name)

    # Check for duplicate names
    if cmd_name in seen_names:
        raise ValueError(
            f"Duplicate command name '{cmd_name}': found in both '{seen_names[cmd_name]}' and '{group_name}'"
        )
    seen_names[cmd_name] = group_name

    # Build the Click command
    cmd = _build_command(info)

    cli.add_command(cmd)
    cli.command_groups[cmd_name] = group_name


def main(
    name: str | None = None,
    *,
    python_cmd: str = "python",
    working_directory: str | None = None,
    commands: Sequence[CommandGroup | TaskWrapper[Any, Any]],
    automations: Sequence[Any] | None = None,
    dispatchables: Sequence[Any] | None = None,
    module_name: str | None = None,
) -> None:
    """
    Build and run the CLI with explicit command registration.

    Args:
        name: Optional name for the CLI group. Defaults to the script name.
        python_cmd: Command to invoke Python in generated GHA workflows.
        working_directory: Working directory for GHA workflows (relative to repo root).
        commands: List of CommandGroups or tasks to expose as CLI commands.
        automations: List of automations to register for GHA workflow generation.
        dispatchables: List of dispatchable tasks for GHA workflow generation.
        module_name: Importable module path for subprocess isolation.
                    If not provided, auto-detected from caller frame.

    Example
    -------
        commands = [
            recompose.CommandGroup("Quality", [lint, format_check]),
            recompose.CommandGroup("Testing", [test]),
            recompose.builtin_commands(),
        ]
        recompose.main(python_cmd="uv run python", commands=commands)

    """
    import sys

    # Store config for GHA workflow generation
    set_python_cmd(python_cmd)
    set_working_directory(working_directory)

    # Set module name (for subprocess isolation)
    if module_name is not None:
        set_module_name(module_name)
    else:
        # Auto-detect from caller frame
        caller_frame = sys._getframe(1)
        caller_spec = caller_frame.f_globals.get("__spec__")

        if caller_spec is not None and caller_spec.name:
            set_module_name(caller_spec.name)
        else:
            raise ValueError(
                "Could not detect module name. Run with `python -m <module>` "
                "or use recompose.App which handles this automatically."
            )

    # Build the registry from commands and automations
    recompose_ctx = _build_registry(commands, automations or [], dispatchables or [])
    set_recompose_context(recompose_ctx)

    # Build and run the CLI
    cli = _build_grouped_cli(name, commands)
    cli()


def _build_registry(
    commands: Sequence[CommandGroup | TaskWrapper[Any, Any]],
    automations: Sequence[Any],
    dispatchables: Sequence[Any],
) -> RecomposeContext:
    """
    Build a RecomposeContext from the commands and automations lists.

    Extracts TaskInfo from the wrappers and populates the registries.
    """
    from .jobs import AutomationInfo

    tasks: dict[str, TaskInfo] = {}
    automation_registry: dict[str, AutomationInfo] = {}

    # Extract tasks from commands
    for item in commands:
        if isinstance(item, CommandGroup):
            for cmd_wrapper in item.commands:
                _register_command(cmd_wrapper, tasks)
        else:
            _register_command(item, tasks)

    # Extract automations
    for auto in automations:
        if hasattr(auto, "_automation_info"):
            info = auto._automation_info
            automation_registry[info.full_name] = info

    # Note: dispatchables are handled by builtin_tasks.generate_gha
    # They don't need to be in the registry since they're passed directly

    return RecomposeContext(
        tasks=tasks,
        automations=automation_registry,
    )


def _register_command(
    cmd_wrapper: TaskWrapper[Any, Any],
    tasks: dict[str, TaskInfo],
) -> None:
    """Register a task in the registry."""
    if hasattr(cmd_wrapper, "_task_info"):
        task_info = cast(TaskWrapper[Any, Any], cmd_wrapper)._task_info
        tasks[task_info.full_name] = task_info
