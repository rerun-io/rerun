"""CLI generation for recompose tasks."""

from __future__ import annotations

import inspect
import time
from enum import Enum
from pathlib import Path
from typing import Any, get_args, get_origin

import click
from rich.console import Console

from .context import set_debug, set_entry_point, set_python_cmd, set_working_directory
from .flow import FlowInfo, get_flow_registry
from .result import Result
from .task import TaskInfo, get_registry

console = Console()


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

    for param_name, param in sig.parameters.items():
        if param_name == "self":
            continue

        # Get type annotation
        annotation = param.annotation
        if annotation is inspect.Parameter.empty:
            annotation = str  # Default to string if no annotation

        click_type, type_required = _get_click_type(annotation)

        # Check if there's a default
        has_default = param.default is not inspect.Parameter.empty
        default_value = param.default if has_default else None

        # Determine if required
        required = not has_default and type_required

        # Handle bool specially (use flag style)
        if annotation is bool:
            if has_default and default_value is True:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
                        default=True,
                        help="(default: True)",
                    )
                )
            elif has_default and default_value is False:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
                        default=False,
                        help="(default: False)",
                    )
                )
            else:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
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
                    [f"--{param_name}"],
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

    # Build the command
    cmd = click.Command(
        name=task_info.name,
        callback=callback,
        params=params,
        help=task_info.doc,
    )

    return cmd


def _build_flow_command(flow_info: FlowInfo) -> click.Command:
    """Build a Click command from a flow."""
    import sys

    from .workspace import (
        FlowParams,
        create_workspace,
        get_workspace_from_env,
        read_params,
        read_step_result,
        write_params,
        write_step_result,
    )

    sig = flow_info.signature
    params: list[click.Parameter] = []

    # Add flow-specific options for subprocess isolation
    params.append(
        click.Option(
            ["--setup"],
            is_flag=True,
            default=False,
            help="Initialize workspace only, don't run (for CI orchestration)",
        )
    )
    params.append(
        click.Option(
            ["--step"],
            type=str,
            default=None,
            help="Execute a single step only (for CI orchestration)",
        )
    )
    params.append(
        click.Option(
            ["--workspace"],
            type=click.Path(path_type=Path),
            default=None,
            help="Workspace directory for step results (default: auto-generated in ~/.recompose/runs/)",
        )
    )

    # Add flow parameters
    for param_name, param in sig.parameters.items():
        if param_name == "self":
            continue

        # Get type annotation
        annotation = param.annotation
        if annotation is inspect.Parameter.empty:
            annotation = str

        click_type, type_required = _get_click_type(annotation)

        has_default = param.default is not inspect.Parameter.empty
        default_value = param.default if has_default else None
        required = not has_default and type_required

        if annotation is bool:
            if has_default and default_value is True:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
                        default=True,
                        help="(default: True)",
                    )
                )
            elif has_default and default_value is False:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
                        default=False,
                        help="(default: False)",
                    )
                )
            else:
                params.append(
                    click.Option(
                        [f"--{param_name}/--no-{param_name}"],
                        default=False,
                        required=required,
                    )
                )
        else:
            help_text = None
            if has_default and default_value is not None:
                help_text = f"(default: {default_value})"

            option_kwargs: dict[str, Any] = {
                "type": click_type,
                "required": required,
                "help": help_text,
            }
            if has_default:
                option_kwargs["default"] = default_value

            params.append(
                click.Option(
                    [f"--{param_name}"],
                    **option_kwargs,
                )
            )

    def callback(setup: bool, step: str | None, workspace: Path | None, **kwargs: Any) -> None:
        """Execute the flow, setup, or a specific step."""
        from datetime import datetime

        flow_name = flow_info.name

        # Convert enum values back to enum if needed
        for param_name, param in sig.parameters.items():
            if param_name in kwargs:
                annotation = param.annotation
                if isinstance(annotation, type) and issubclass(annotation, Enum):
                    value = kwargs[param_name]
                    if value is not None:
                        kwargs[param_name] = annotation(value)

        # Determine workspace
        ws = workspace or get_workspace_from_env()

        if setup:
            # --setup mode: Create workspace and write params
            if ws is None:
                ws = create_workspace(flow_name)

            # Build the plan to get step names
            plan = flow_info.fn.plan(**kwargs)  # type: ignore[attr-defined]
            plan.assign_step_names()

            step_names = [n.step_name for n in plan.get_execution_order() if n.step_name]

            flow_params = FlowParams(
                flow_name=flow_name,
                params=kwargs,
                steps=step_names,
                created_at=datetime.now().isoformat(),
                script_path=sys.argv[0],
            )
            write_params(ws, flow_params)

            console.print(f"\n[bold green]✓[/bold green] Setup complete for [bold]{flow_name}[/bold]")
            console.print(f"[dim]Workspace:[/dim] {ws}")
            console.print("[dim]Steps:[/dim]")
            for s in step_names:
                console.print(f"    {s}")
            console.print()

        elif step:
            # --step mode: Execute a specific step
            if ws is None:
                ws = get_workspace_from_env()
                if ws is None:
                    console.print("[red]Error:[/red] --workspace required or set $RECOMPOSE_WORKSPACE")
                    sys.exit(1)

            # Read params from workspace
            try:
                flow_params = read_params(ws)
            except FileNotFoundError:
                console.print(f"[red]Error:[/red] No _params.json in {ws}")
                console.print("[dim]Run --setup first to initialize the workspace[/dim]")
                sys.exit(1)

            # Rebuild the plan using stored params
            plan = flow_info.fn.plan(**flow_params.params)  # type: ignore[attr-defined]
            plan.assign_step_names()

            # Find the requested step
            target_node = plan.get_step(step)
            if target_node is None:
                console.print(f"[red]Error:[/red] Step '{step}' not found")
                console.print("[dim]Available steps:[/dim]")
                for s in flow_params.steps:
                    console.print(f"    {s}")
                sys.exit(1)

            step_name = target_node.step_name or target_node.name

            console.print(f"\n[bold cyan]▶[/bold cyan] [bold]{step_name}[/bold]")
            console.print()

            # Resolve dependencies from workspace
            resolved_kwargs: dict[str, Any] = {}
            for kwarg_name, kwarg_value in target_node.kwargs.items():
                if isinstance(kwarg_value, type(target_node)):  # TaskNode dependency
                    dep_node = kwarg_value
                    dep_step_name = dep_node.step_name or dep_node.name
                    dep_result = read_step_result(ws, dep_step_name)
                    if dep_result.failed:
                        console.print(f"[red]Error:[/red] Dependency '{dep_step_name}' failed or not found")
                        sys.exit(1)
                    resolved_kwargs[kwarg_name] = dep_result.value()
                else:
                    resolved_kwargs[kwarg_name] = kwarg_value

            # Execute the task
            start_time = time.perf_counter()
            result: Result[Any] = target_node.task_info.original_fn(**resolved_kwargs)
            elapsed = time.perf_counter() - start_time

            # Write result to workspace
            write_step_result(ws, step_name, result)

            # Print result
            if result.ok:
                console.print(f"[bold green]✓[/bold green] [bold]{step_name}[/bold] succeeded in {elapsed:.2f}s")
                if result._value is not None:
                    console.print(f"[dim]→[/dim] {result._value}")
            else:
                console.print(f"[bold red]✗[/bold red] [bold]{step_name}[/bold] failed in {elapsed:.2f}s")
                if result.error:
                    console.print(f"[red]Error:[/red] {result.error}")
                sys.exit(1)

            console.print()

        else:
            # Normal mode: Execute the entire flow with subprocess isolation
            # This matches CI behavior where each step is a separate process
            start_time = time.perf_counter()

            console.print(f"\n[bold magenta]▶[/bold magenta] [bold]flow:{flow_name}[/bold]")
            console.print()

            result = flow_info.fn.run_isolated(workspace=ws, **kwargs)  # type: ignore[attr-defined]

            elapsed = time.perf_counter() - start_time

            console.print()
            if result.ok:
                console.print(f"[bold green]✓[/bold green] [bold]flow:{flow_name}[/bold] succeeded in {elapsed:.2f}s")
            else:
                console.print(f"[bold red]✗[/bold red] [bold]flow:{flow_name}[/bold] failed in {elapsed:.2f}s")
                if result.error:
                    console.print(f"[red]Error:[/red] {result.error}")

            console.print()

    cmd = click.Command(
        name=flow_info.name,
        callback=callback,
        params=params,
        help=f"[flow] {flow_info.doc}" if flow_info.doc else "[flow]",
    )

    return cmd


def main(
    name: str | None = None,
    python_cmd: str = "python",
    working_directory: str | None = None,
) -> None:
    """
    Build and run the CLI from registered tasks.

    Call this at the end of your script to expose all registered tasks as CLI commands.

    Args:
        name: Optional name for the CLI group. Defaults to the script name.
        python_cmd: Command to invoke Python in generated GHA workflows.
                   Use "uv run python" for uv-managed projects.
        working_directory: Working directory for GHA workflows (relative to repo root).
                          If set, workflows will cd to this directory before running.

    """
    import sys

    # Store config for GHA workflow generation
    set_python_cmd(python_cmd)
    set_working_directory(working_directory)

    # Detect if we're running as a module (python -m) or as a script
    # When running as a module, __spec__ is set in the calling module
    caller_frame = sys._getframe(1)
    caller_spec = caller_frame.f_globals.get("__spec__")

    if caller_spec is not None and caller_spec.name:
        # Running as a module - store module name for -m invocation
        set_entry_point("module", caller_spec.name)
    else:
        # Running as a script - store the script path
        set_entry_point("script", sys.argv[0])

    @click.group(name=name)
    @click.option("--debug/--no-debug", default=False, help="Enable debug output")
    @click.pass_context
    def cli(ctx: click.Context, debug: bool) -> None:
        """Recompose task runner."""
        ctx.ensure_object(dict)
        set_debug(debug)

    # Add a command for each registered task
    registry = get_registry()
    for _task_key, task_info in registry.items():
        cmd = _build_command(task_info)
        cli.add_command(cmd)

    # Add a command for each registered flow
    flow_registry = get_flow_registry()
    for _flow_key, flow_info in flow_registry.items():
        cmd = _build_flow_command(flow_info)
        cli.add_command(cmd)

    # Run the CLI
    cli()
