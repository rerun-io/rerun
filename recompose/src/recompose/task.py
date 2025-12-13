"""Task decorator and registry for recompose."""

from __future__ import annotations

import functools
import inspect
import traceback
from dataclasses import dataclass
from typing import Any, Callable, ParamSpec, TypeVar

from .context import Context, get_context, set_context
from .result import Err, Result

P = ParamSpec("P")
T = TypeVar("T")


@dataclass
class TaskInfo:
    """Metadata about a registered task."""

    name: str
    module: str
    fn: Callable  # The wrapped function (with context/exception handling)
    original_fn: Callable  # The original unwrapped function
    signature: inspect.Signature
    doc: str | None

    @property
    def full_name(self) -> str:
        """Full qualified name of the task."""
        return f"{self.module}:{self.name}"


# Global registry of all tasks
_task_registry: dict[str, TaskInfo] = {}


def get_registry() -> dict[str, TaskInfo]:
    """Get the task registry."""
    return _task_registry


def get_task(name: str) -> TaskInfo | None:
    """Get a task by name. Tries full name first, then short name."""
    # Try exact match first
    if name in _task_registry:
        return _task_registry[name]

    # Try matching by short name
    for full_name, info in _task_registry.items():
        if info.name == name:
            return info

    return None


def task(fn: Callable[P, Result[T]]) -> Callable[P, Result[T]]:
    """
    Decorator to mark a function as a recompose task.

    The decorated function:
    - Is registered in the global task registry
    - Gets automatic context management
    - Has exceptions caught and converted to Err results
    - Can still be called as a normal Python function
    """
    @functools.wraps(fn)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> Result[T]:
        # Check if we're already in a context
        existing_ctx = get_context()

        if existing_ctx is None:
            # Create a new context for this task
            ctx = Context(task_name=info.name)
            set_context(ctx)
            try:
                return _execute_task(fn, args, kwargs)
            finally:
                set_context(None)
        else:
            # Already in a context, just execute
            return _execute_task(fn, args, kwargs)

    # Create task info with the wrapper
    info = TaskInfo(
        name=fn.__name__,
        module=fn.__module__,
        fn=wrapper,  # Store the wrapper
        original_fn=fn,  # Keep reference to original
        signature=inspect.signature(fn),
        doc=fn.__doc__,
    )
    _task_registry[info.full_name] = info

    # Attach task info to wrapper for introspection
    wrapper._task_info = info  # type: ignore[attr-defined]

    return wrapper


def _execute_task(fn: Callable, args: tuple, kwargs: dict) -> Result[Any]:
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
