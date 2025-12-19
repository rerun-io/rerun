"""CLI generation for recompose tasks."""

from __future__ import annotations

import inspect
import os
import sys
import time
from collections.abc import Sequence
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast, get_args, get_origin

import click

from .command_group import CommandGroup
from .context import (
    RecomposeContext,
    set_cli_command,
    set_debug,
    set_module_name,
    set_recompose_context,
    set_working_directory,
)
from .output import get_output_manager
from .result import Result
from .task import TaskInfo, TaskWrapper

if TYPE_CHECKING:
    from .jobs import AutomationInfo, AutomationWrapper


def _get_console():
    """Get console from OutputManager to respect NO_COLOR settings."""
    return get_output_manager().console


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

        # Check if running as subprocess of automation (suppress headers)
        # Clear it immediately so it doesn't propagate to grandchild processes
        quiet_mode = os.environ.pop("RECOMPOSE_SUBPROCESS", None) == "1"

        # Start timing
        start_time = time.perf_counter()

        # Print task header (unless in quiet mode)
        if not quiet_mode:
            _get_console().print(f"\n[bold blue]▶[/bold blue] [bold]{task_name}[/bold]")
            _get_console().print()

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

        # Print result (unless in quiet mode)
        if not quiet_mode:
            _get_console().print()
            if result.ok:
                _get_console().print(f"[bold green]✓[/bold green] [bold]{task_name}[/bold] succeeded in {elapsed:.2f}s")
                if result._value is not None:
                    _get_console().print(f"[dim]→[/dim] {result._value}")
            else:
                _get_console().print(f"[bold red]✗[/bold red] [bold]{task_name}[/bold] failed in {elapsed:.2f}s")
                if result.error:
                    _get_console().print(f"[red]Error:[/red] {result.error}")
                if result.traceback:
                    from .context import is_debug

                    if is_debug():
                        _get_console().print(f"[dim]{result.traceback}[/dim]")

            _get_console().print()

        # Exit with non-zero code if task failed
        if not result.ok:
            sys.exit(1)

    # Build the command with kebab-case name
    cmd = click.Command(
        name=_to_kebab_case(task_info.name),
        callback=callback,
        params=params,
        help=task_info.doc,
    )

    return cmd


def _build_automation_command(
    automation_wrapper: AutomationWrapper,
    cli_command: str,
    working_directory: str | None,
) -> click.Command:
    """Build a Click command from an automation."""

    info = automation_wrapper.info
    params: list[click.Parameter] = []

    # Add common automation options
    params.append(
        click.Option(
            ["--dry-run"],
            is_flag=True,
            default=False,
            help="Show what would be run without executing",
        )
    )
    params.append(
        click.Option(
            ["--verbose", "-v"],
            is_flag=True,
            default=False,
            help="Show verbose output",
        )
    )

    # Add options for each InputParam
    for param_name, input_param in info.input_params.items():
        cli_name = _to_kebab_case(param_name)
        has_default = input_param._default is not None
        required = input_param._required

        # Determine type from choices or default
        if input_param._choices:
            click_type: Any = click.Choice(input_param._choices)
        elif isinstance(input_param._default, bool):
            # Bool flag
            params.append(
                click.Option(
                    [f"--{cli_name}/--no-{cli_name}"],
                    default=input_param._default,
                    help=input_param._description or f"(default: {input_param._default})",
                )
            )
            continue
        elif isinstance(input_param._default, int):
            click_type = click.INT
        elif isinstance(input_param._default, float):
            click_type = click.FLOAT
        else:
            click_type = click.STRING

        option_kwargs: dict[str, Any] = {
            "type": click_type,
            "required": required,
            "help": input_param._description,
        }
        if has_default:
            option_kwargs["default"] = input_param._default

        params.append(click.Option([f"--{cli_name}"], **option_kwargs))

    def callback(dry_run: bool, verbose: bool, **kwargs: Any) -> None:
        """Execute the automation locally."""
        from .local_executor import LocalExecutor

        # Convert kebab-case back to snake_case for kwargs
        input_params = {}
        for param_name in info.input_params:
            cli_name = _to_kebab_case(param_name)
            if cli_name in kwargs:
                input_params[param_name] = kwargs[cli_name]
            elif param_name in kwargs:
                input_params[param_name] = kwargs[param_name]

        executor = LocalExecutor(
            cli_command=cli_command,
            working_directory=working_directory,
            dry_run=dry_run,
            verbose=verbose,
        )

        result = executor.execute(automation_wrapper, **input_params)

        if not result.success:
            sys.exit(1)

    # Build the command
    cmd = click.Command(
        name=_to_kebab_case(info.name),
        callback=callback,
        params=params,
        help=info.doc or f"Run the {info.name} automation locally",
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
    automations: Sequence[Any] | None = None,
    cli_command: str = "./run",
    working_directory: str | None = None,
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

    # Add automation commands (skip those that conflict with task names)
    if automations:
        for auto in automations:
            if hasattr(auto, "_automation_info"):
                info = auto._automation_info
                cmd_name = _to_kebab_case(info.name)
                # Skip if this automation has the same name as an existing task
                # (e.g., make_dispatchable(lint) conflicts with lint task)
                # Running the task directly is equivalent to running the dispatchable locally
                if cmd_name in seen_names:
                    continue
                _add_automation_to_cli(cli, auto, "Automations", seen_names, cli_command, working_directory)

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


def _add_automation_to_cli(
    cli: GroupedClickGroup,
    automation_wrapper: AutomationWrapper,
    group_name: str,
    seen_names: dict[str, str],
    cli_command: str,
    working_directory: str | None,
) -> None:
    """Add an automation to the CLI, checking for duplicates."""
    info = automation_wrapper.info

    # Use kebab-case for CLI command names
    cmd_name = _to_kebab_case(info.name)

    # Check for duplicate names
    if cmd_name in seen_names:
        raise ValueError(
            f"Duplicate command name '{cmd_name}': found in both '{seen_names[cmd_name]}' and '{group_name}'"
        )
    seen_names[cmd_name] = group_name

    # Build the Click command
    cmd = _build_automation_command(automation_wrapper, cli_command, working_directory)

    cli.add_command(cmd)
    cli.command_groups[cmd_name] = group_name


def main(
    name: str | None = None,
    *,
    cli_command: str = "./run",
    working_directory: str | None = None,
    commands: Sequence[CommandGroup | TaskWrapper[Any, Any]],
    automations: Sequence[Any] | None = None,
    module_name: str | None = None,
) -> None:
    """
    Build and run the CLI with explicit command registration.

    Args:
        name: Optional name for the CLI group. Defaults to the script name.
        cli_command: CLI entry point for generated GHA workflows (default: "./run").
        working_directory: Working directory for GHA workflows (relative to repo root).
        commands: List of CommandGroups or tasks to expose as CLI commands.
        automations: List of automations to register for GHA workflow generation.
                    Dispatchables (from make_dispatchable) are also automations.
        module_name: Importable module path for subprocess isolation.
                    If not provided, auto-detected from caller frame.

    Example
    -------
        commands = [
            recompose.CommandGroup("Quality", [lint, format_check]),
            recompose.CommandGroup("Testing", [test]),
            recompose.builtin_commands(),
        ]
        recompose.main(cli_command="./run", commands=commands)

    """
    # Store config for GHA workflow generation
    set_cli_command(cli_command)
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
    recompose_ctx = _build_registry(commands, automations or [])
    set_recompose_context(recompose_ctx)

    # Build and run the CLI (including automation commands)
    cli = _build_grouped_cli(
        name,
        commands,
        automations=automations,
        cli_command=cli_command,
        working_directory=working_directory,
    )
    cli()


def _build_registry(
    commands: Sequence[CommandGroup | TaskWrapper[Any, Any]],
    automations: Sequence[Any],
) -> RecomposeContext:
    """
    Build a RecomposeContext from the commands and automations lists.

    Extracts TaskInfo from the wrappers and populates the registries.
    Note: Dispatchables (from make_dispatchable) are automations with workflow_dispatch trigger.
    """

    tasks: dict[str, TaskInfo] = {}
    automation_registry: dict[str, AutomationInfo] = {}

    # Extract tasks from commands
    for item in commands:
        if isinstance(item, CommandGroup):
            for cmd_wrapper in item.commands:
                _register_command(cmd_wrapper, tasks)
        else:
            _register_command(item, tasks)

    # Extract automations (includes dispatchables, which are also AutomationWrappers)
    for auto in automations:
        if hasattr(auto, "_automation_info"):
            info = auto._automation_info
            automation_registry[info.full_name] = info

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
