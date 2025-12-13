"""CLI generation for recompose tasks."""

from __future__ import annotations

import inspect
import time
from enum import Enum
from pathlib import Path
from typing import Any, get_args, get_origin

import click
from rich.console import Console

from .context import set_debug
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
            console.print(
                f"[bold green]✓[/bold green] [bold]{task_name}[/bold] succeeded in {elapsed:.2f}s"
            )
            if result.value is not None:
                console.print(f"[dim]→[/dim] {result.value}")
        else:
            console.print(
                f"[bold red]✗[/bold red] [bold]{task_name}[/bold] failed in {elapsed:.2f}s"
            )
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
    sig = flow_info.signature
    params: list[click.Parameter] = []

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

    def callback(**kwargs: Any) -> None:
        """Execute the flow and display results."""
        flow_name = flow_info.name

        start_time = time.perf_counter()

        # Print flow header
        console.print(f"\n[bold magenta]▶[/bold magenta] [bold]flow:{flow_name}[/bold]")
        console.print()

        # Convert enum values back to enum if needed
        for param_name, param in sig.parameters.items():
            if param_name in kwargs:
                annotation = param.annotation
                if isinstance(annotation, type) and issubclass(annotation, Enum):
                    value = kwargs[param_name]
                    if value is not None:
                        kwargs[param_name] = annotation(value)

        # Execute the flow
        result: Result[Any] = flow_info.fn(**kwargs)

        # Get flow context from result (attached by the flow decorator)
        flow_ctx = getattr(result, "_flow_context", None)

        elapsed = time.perf_counter() - start_time

        # Print sub-task summary if available
        if flow_ctx and flow_ctx.executions:
            console.print()
            console.print("[dim]Tasks executed:[/dim]")
            for ex in flow_ctx.executions:
                status_icon = "[green]✓[/green]" if ex.result.ok else "[red]✗[/red]"
                console.print(f"  {status_icon} {ex.task_name} ({ex.duration:.2f}s)")

        # Print result
        console.print()
        if result.ok:
            console.print(
                f"[bold green]✓[/bold green] [bold]flow:{flow_name}[/bold] succeeded in {elapsed:.2f}s"
            )
            if result.value is not None:
                console.print(f"[dim]→[/dim] {result.value}")
        else:
            console.print(
                f"[bold red]✗[/bold red] [bold]flow:{flow_name}[/bold] failed in {elapsed:.2f}s"
            )
            if result.error:
                console.print(f"[red]Error:[/red] {result.error}")
            if result.traceback:
                from .context import is_debug

                if is_debug():
                    console.print(f"[dim]{result.traceback}[/dim]")

        console.print()

    cmd = click.Command(
        name=flow_info.name,
        callback=callback,
        params=params,
        help=f"[flow] {flow_info.doc}" if flow_info.doc else "[flow]",
    )

    return cmd


def _inspect_task(task_info: TaskInfo) -> None:
    """Display information about a task."""
    console.print(f"\n[bold cyan]Task:[/bold cyan] [bold]{task_info.name}[/bold]")
    console.print(f"[dim]Module:[/dim] {task_info.module}")

    if task_info.doc:
        console.print("\n[dim]Description:[/dim]")
        console.print(f"  {task_info.doc}")

    console.print("\n[dim]Signature:[/dim]")
    params = []
    for param_name, param in task_info.signature.parameters.items():
        if param_name == "self":
            continue
        annotation = param.annotation
        type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
        if param.default is not inspect.Parameter.empty:
            params.append(f"  --{param_name}: {type_str} = {param.default!r}")
        else:
            params.append(f"  --{param_name}: {type_str} [required]")

    if params:
        for p in params:
            console.print(p)
    else:
        console.print("  (no parameters)")

    console.print()


def _inspect_flow(flow_info: FlowInfo, kwargs: dict[str, Any]) -> None:
    """Display information about a flow, including its task graph."""
    console.print(f"\n[bold magenta]Flow:[/bold magenta] [bold]{flow_info.name}[/bold]")
    console.print(f"[dim]Module:[/dim] {flow_info.module}")

    if flow_info.doc:
        console.print("\n[dim]Description:[/dim]")
        # Print first line of docstring
        first_line = flow_info.doc.strip().split("\n")[0]
        console.print(f"  {first_line}")

    console.print("\n[dim]Parameters:[/dim]")
    params = []
    for param_name, param in flow_info.signature.parameters.items():
        annotation = param.annotation
        type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
        if param.default is not inspect.Parameter.empty:
            params.append(f"  --{param_name}: {type_str} = {param.default!r}")
        else:
            params.append(f"  --{param_name}: {type_str} [required]")

    if params:
        for p in params:
            console.print(p)
    else:
        console.print("  (no parameters)")

    # Build the flow plan to inspect
    try:
        plan = flow_info.fn.plan(**kwargs)  # type: ignore[attr-defined]

        console.print(f"\n[dim]Task Graph ({len(plan.nodes)} tasks):[/dim]")

        # Show execution order
        console.print("\n  [bold]Execution order:[/bold]")
        for i, node in enumerate(plan.get_execution_order(), 1):
            deps = [d.name for d in node.dependencies]
            if deps:
                console.print(f"    {i}. {node.name} [dim]← {deps}[/dim]")
            else:
                console.print(f"    {i}. {node.name}")

        # Show parallelizable groups
        groups = plan.get_parallelizable_groups()
        if len(groups) > 1:
            console.print("\n  [bold]Parallelizable groups:[/bold]")
            for level, group in enumerate(groups):
                names = [n.name for n in group]
                if len(names) > 1:
                    console.print(f"    Level {level}: [green]{', '.join(names)}[/green] (can run in parallel)")
                else:
                    console.print(f"    Level {level}: {names[0]}")

        if plan.terminal:
            console.print(f"\n  [bold]Terminal task:[/bold] {plan.terminal.name}")

    except Exception as e:
        console.print(f"\n[yellow]Could not build flow plan: {e}[/yellow]")
        console.print("[dim]Some flow parameters may be required to inspect the graph.[/dim]")

    console.print()


def main(name: str | None = None) -> None:
    """
    Build and run the CLI from registered tasks.

    Call this at the end of your script to expose all registered tasks as CLI commands.

    Args:
        name: Optional name for the CLI group. Defaults to the script name.
    """

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

    # Add inspect command
    @cli.command("inspect")
    @click.argument("target")
    @click.option("--param", "-p", multiple=True, help="Parameters for flow inspection (key=value)")
    def inspect_command(target: str, param: tuple[str, ...]) -> None:
        """Inspect a task or flow without executing it.

        Shows task/flow signature, documentation, and for flows, the task dependency graph.

        Examples:

            ./app.py inspect my_task

            ./app.py inspect my_flow

            ./app.py inspect my_flow -p repo=main -p branch=dev
        """
        # Parse params into kwargs
        kwargs: dict[str, Any] = {}
        for p in param:
            if "=" in p:
                key, value = p.split("=", 1)
                kwargs[key] = value
            else:
                console.print(f"[red]Invalid parameter format: {p}[/red]")
                console.print("[dim]Use key=value format[/dim]")
                return

        # Try to find as a task first
        from .task import get_task

        task_info = get_task(target)
        if task_info is not None:
            _inspect_task(task_info)
            return

        # Try to find as a flow
        from .flow import get_flow

        flow_info = get_flow(target)
        if flow_info is not None:
            _inspect_flow(flow_info, kwargs)
            return

        # Not found
        console.print(f"[red]Unknown task or flow: {target}[/red]")
        console.print("\n[dim]Available tasks:[/dim]")
        for t_info in registry.values():
            console.print(f"  {t_info.name}")
        console.print("\n[dim]Available flows:[/dim]")
        for f_info in flow_registry.values():
            console.print(f"  {f_info.name}")
        console.print()

    # Run the CLI
    cli()
