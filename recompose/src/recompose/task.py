"""Task decorator and registry for recompose."""

from __future__ import annotations

import functools
import inspect
import os
import traceback
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, ParamSpec, Protocol, TypeVar, overload

from .context import Context, decrement_task_depth, get_context, increment_task_depth, set_context
from .result import Err, Result

P = ParamSpec("P")
T = TypeVar("T")


class TaskWrapper(Protocol[P, T]):
    """
    Protocol describing a task-decorated function.

    Task wrappers are callable and return Result[T] when executed.
    """

    _task_info: TaskInfo

    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> Result[T]: ...


@dataclass
class TaskInfo:
    """Metadata about a registered task."""

    name: str
    module: str
    fn: Callable[..., Any]  # The wrapped function (with context/exception handling)
    original_fn: Callable[..., Any]  # The original unwrapped function
    signature: inspect.Signature
    doc: str | None

    # GHA action fields (for virtual tasks that map to `uses:` steps)
    is_gha_action: bool = False  # True if this is a GHA virtual action
    gha_uses: str | None = None  # The action to use, e.g., "actions/checkout@v4"

    # P14 fields: outputs, artifacts, secrets, setup
    outputs: list[str] = field(default_factory=list)  # Declared output names
    artifacts: list[str] = field(default_factory=list)  # Declared artifact names
    secrets: list[str] = field(default_factory=list)  # Declared secret names
    setup: list[Any] | None = None  # Setup steps (overrides app-level defaults)

    @property
    def full_name(self) -> str:
        """Full qualified name of the task."""
        return f"{self.module}:{self.name}"


def _is_method_signature(fn: Callable[..., Any]) -> bool:
    """Check if a function signature indicates it's a method (first param is 'self')."""
    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    return len(params) > 0 and params[0] == "self"


@overload
def task(fn: Callable[P, Result[T]]) -> TaskWrapper[P, T]: ...


@overload
def task(
    *,
    outputs: list[str] | None = None,
    artifacts: list[str] | None = None,
    secrets: list[str] | None = None,
    setup: list[Any] | None = None,
) -> Callable[[Callable[P, Result[T]]], TaskWrapper[P, T]]: ...


def task(
    fn: Callable[P, Result[T]] | None = None,
    *,
    outputs: list[str] | None = None,
    artifacts: list[str] | None = None,
    secrets: list[str] | None = None,
    setup: list[Any] | None = None,
) -> TaskWrapper[P, T] | Callable[[Callable[P, Result[T]]], TaskWrapper[P, T]]:
    """
    Decorator to mark a function as a recompose task.

    The decorated function:
    - Gets automatic context management
    - Has exceptions caught and converted to Err results

    Note: Tasks are NOT automatically registered. To expose a task as a CLI
    command, include it in the `commands` parameter to `recompose.main()`.

    Args:
        outputs: List of output names this task can set via set_output().
        artifacts: List of artifact names this task can save via save_artifact().
        secrets: List of secret names this task requires via get_secret().
        setup: Setup steps for GHA (overrides app-level defaults).

    Usage:
        @task
        def compile(*, source: Path) -> Result[Path]:
            ...

        @task(outputs=["wheel_path", "version"])
        def build_wheel() -> Result[None]:
            recompose.set_output("wheel_path", "/dist/pkg-1.0.0.whl")
            recompose.set_output("version", "1.0.0")
            return Ok(None)

        @task(secrets=["PYPI_TOKEN"])
        def publish() -> Result[None]:
            token = recompose.get_secret("PYPI_TOKEN")
            # ... use token
            return Ok(None)

        # Direct execution:
        result = compile(source=Path("src/"))  # Returns Result[Path]

    """

    def decorator(fn: Callable[P, Result[T]]) -> TaskWrapper[P, T]:
        # Check if this looks like a method - error as @task is for standalone functions
        if _is_method_signature(fn):
            raise TypeError(
                f"@task cannot be used on methods (found 'self' parameter in {fn.__name__}). "
                f"Define tasks as standalone functions instead."
            )

        @functools.wraps(fn)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> Result[T]:
            return _run_with_context(info, fn, args, kwargs)

        # Create task info with the wrapper
        info = TaskInfo(
            name=fn.__name__,
            module=fn.__module__,
            fn=wrapper,  # Store the wrapper
            original_fn=fn,  # Keep reference to original
            signature=inspect.signature(fn),
            doc=fn.__doc__,
            outputs=outputs or [],
            artifacts=artifacts or [],
            secrets=secrets or [],
            setup=setup,
        )

        # Attach task info to wrapper for introspection
        wrapper._task_info = info  # type: ignore[attr-defined]

        # Cast to TaskWrapper to satisfy type checker
        from typing import cast

        return cast(TaskWrapper[P, T], wrapper)

    # Handle both @task and @task(...) forms
    if fn is not None:
        return decorator(fn)
    return decorator


def _execute_task(fn: Callable[..., Any], args: tuple[Any, ...], kwargs: dict[str, Any]) -> Result[Any]:
    """Execute a task function, catching exceptions."""
    try:
        result = fn(*args, **kwargs)

        # Ensure the result is a Result type
        if not isinstance(result, Result):
            # If the function didn't return a Result, wrap it
            from .result import Ok

            return Ok(result)

        return result

    except Exception as e:
        # Catch any exception and convert to Err
        tb = traceback.format_exc()
        return Err(f"{type(e).__name__}: {e}", traceback=tb)


def _run_with_context(
    task_info: TaskInfo, fn: Callable[..., Any], args: tuple[Any, ...], kwargs: dict[str, Any]
) -> Result[Any]:
    """
    Execute task with context management and tree-style output.

    Uses recursive capture-and-prefix model:
    1. Print task name (with marker if nested, plain if top-level)
    2. Capture ALL body output
    3. Prefix captured output appropriately
    4. Print status

    In GitHub Actions, first-level subtasks emit ::group:: / ::endgroup:: markers.
    """
    import io
    import sys
    import time

    from rich.console import Console

    from .output import COLORS, SUBTASK_MARKER, SYMBOLS, prefix_task_output, print_task_output_styled

    existing_ctx = get_context()
    task_name = task_info.name
    start_time = time.perf_counter()
    # force_terminal=True ensures ANSI codes are output even when captured
    console = Console(force_terminal=True)

    # Track nesting depth (depth after increment: 1=top-level, 2=first subtask, etc.)
    depth = increment_task_depth()
    is_gha = os.environ.get("GITHUB_ACTIONS") == "true"
    is_subprocess = os.environ.get("RECOMPOSE_SUBPROCESS") == "1"
    is_nested = existing_ctx is not None or is_subprocess
    # Emit GHA groups for first-level subtasks only (depth == 2 after increment)
    emit_gha_group = is_gha and depth == 2 and is_nested

    # 1. Print task name (with marker if nested/subprocess, plain if bare top-level)
    if is_nested:
        # Nested task or subprocess - print with marker for parent to recognize
        # Marker is plain text so prefix_task_output can detect it
        print(f"{SUBTASK_MARKER}{task_name}", flush=True)
        # In GHA, emit group marker for first-level subtasks
        if emit_gha_group:
            print(f"::group::{task_name}", flush=True)
    else:
        # Bare top-level task - print name in name style
        console.print(task_name, style=COLORS["name"], markup=False, highlight=False)

    # Set up context
    ctx = Context(
        task_name=task_name,
        declared_outputs=task_info.outputs,
        declared_artifacts=task_info.artifacts,
        declared_secrets=task_info.secrets,
    )

    # Only set context if not already in one (avoid overwriting parent context)
    should_set_context = existing_ctx is None
    if should_set_context:
        set_context(ctx)

    try:
        # 2. Capture ALL body output
        buffer = io.StringIO()
        old_stdout = sys.stdout
        old_stderr = sys.stderr
        sys.stdout = buffer
        sys.stderr = buffer

        try:
            result = _execute_task(fn, args, kwargs)
        finally:
            sys.stdout = old_stdout
            sys.stderr = old_stderr

        captured_output = buffer.getvalue()

        # 3. Prefix and print captured output with styled tree prefixes
        # Content may already have ANSI colors which pass through
        if captured_output:
            prefixed = prefix_task_output(captured_output)
            print_task_output_styled(prefixed, console)

        # Print error details if failed
        if not result.ok and result.error:
            error_lines = str(result.error).split("\n")[:5]
            prefixed_error = prefix_task_output("\n".join(error_lines))
            print_task_output_styled(prefixed_error, console)

        # 4. Print status (always styled - ANSI passes through when captured)
        elapsed = time.perf_counter() - start_time
        symbol = SYMBOLS["success"] if result.ok else SYMBOLS["failure"]
        status = "succeeded" if result.ok else "failed"
        style = COLORS["success_bold"] if result.ok else COLORS["failure_bold"]
        msg = f"{symbol} {task_name} {status} in {elapsed:.2f}s"
        console.print(msg, style=style, markup=False, highlight=False)

        # Close GHA group for first-level subtasks
        if emit_gha_group:
            print("::endgroup::", flush=True)

        # Attach collected outputs/artifacts to the result
        if result.ok and should_set_context:
            result = _attach_context_to_result(result, ctx)

        return result
    finally:
        decrement_task_depth()
        if should_set_context:
            set_context(None)


def _attach_context_to_result(result: Result[Any], ctx: Context) -> Result[Any]:
    """Attach outputs and artifacts from context to the result."""
    if ctx.task_outputs or ctx.task_artifacts:
        # Create a new result with outputs/artifacts attached
        object.__setattr__(result, "_outputs", ctx.task_outputs.copy())
        object.__setattr__(result, "_artifacts", ctx.task_artifacts.copy())
    return result
