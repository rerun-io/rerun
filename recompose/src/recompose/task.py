"""Task decorator and registry for recompose."""

from __future__ import annotations

import functools
import inspect
import traceback
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, ParamSpec, Protocol, TypeVar, overload

from .context import Context, get_context, set_context
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
    """Execute task with context management."""
    existing_ctx = get_context()

    if existing_ctx is None:
        ctx = Context(
            task_name=task_info.name,
            declared_outputs=task_info.outputs,
            declared_artifacts=task_info.artifacts,
            declared_secrets=task_info.secrets,
        )
        set_context(ctx)
        try:
            result = _execute_task(fn, args, kwargs)
            # Attach collected outputs/artifacts to the result
            if result.ok:
                result = _attach_context_to_result(result, ctx)
            return result
        finally:
            set_context(None)
    else:
        return _execute_task(fn, args, kwargs)


def _attach_context_to_result(result: Result[Any], ctx: Context) -> Result[Any]:
    """Attach outputs and artifacts from context to the result."""
    if ctx.task_outputs or ctx.task_artifacts:
        # Create a new result with outputs/artifacts attached
        object.__setattr__(result, "_outputs", ctx.task_outputs.copy())
        object.__setattr__(result, "_artifacts", ctx.task_artifacts.copy())
    return result
