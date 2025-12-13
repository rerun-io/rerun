"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
import time
import traceback
from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Any, Callable, ParamSpec, TypeVar

from .context import Context, get_context, set_context
from .result import Err, Ok, Result

P = ParamSpec("P")
T = TypeVar("T")


@dataclass
class TaskExecution:
    """Record of a task execution within a flow."""

    task_name: str
    result: Result[Any]
    duration: float  # seconds


@dataclass
class FlowContext:
    """
    Context for tracking flow execution.

    Tracks which tasks have run and their results.
    """

    flow_name: str
    executions: list[TaskExecution] = field(default_factory=list)
    start_time: float = field(default_factory=time.perf_counter)

    def record_task(self, task_name: str, result: Result[Any], duration: float) -> None:
        """Record a task execution."""
        self.executions.append(TaskExecution(task_name=task_name, result=result, duration=duration))

    @property
    def total_duration(self) -> float:
        """Total elapsed time since flow started."""
        return time.perf_counter() - self.start_time

    @property
    def all_succeeded(self) -> bool:
        """True if all executed tasks succeeded."""
        return all(ex.result.ok for ex in self.executions)


# Context variable for the current flow
_current_flow_context: ContextVar[FlowContext | None] = ContextVar("recompose_flow_context", default=None)


def get_flow_context() -> FlowContext | None:
    """Get the current flow context, or None if not in a flow."""
    return _current_flow_context.get()


def set_flow_context(ctx: FlowContext | None) -> None:
    """Set the current flow context."""
    _current_flow_context.set(ctx)


@dataclass
class FlowInfo:
    """Metadata about a registered flow."""

    name: str
    module: str
    fn: Callable[..., Any]  # The wrapped function
    original_fn: Callable[..., Any]  # The original unwrapped function
    signature: inspect.Signature
    doc: str | None

    @property
    def full_name(self) -> str:
        """Full qualified name of the flow."""
        return f"{self.module}:{self.name}"


# Global registry of all flows
_flow_registry: dict[str, FlowInfo] = {}


def get_flow_registry() -> dict[str, FlowInfo]:
    """Get the flow registry."""
    return _flow_registry


def get_flow(name: str) -> FlowInfo | None:
    """Get a flow by name. Tries full name first, then short name."""
    if name in _flow_registry:
        return _flow_registry[name]

    for full_name, info in _flow_registry.items():
        if info.name == name:
            return info

    return None


def flow(fn: Callable[P, Result[T]]) -> Callable[P, Result[T]]:
    """
    Decorator to mark a function as a recompose flow.

    A flow is a composition of tasks that run sequentially.
    The flow tracks all task executions and their results.

    Example:
        @recompose.flow
        def build_and_test() -> recompose.Result[str]:
            build_result = build_project()
            if build_result.failed:
                return build_result

            test_result = run_tests()
            return test_result
    """

    @functools.wraps(fn)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> Result[T]:
        # Create flow context
        flow_ctx = FlowContext(flow_name=fn.__name__)
        set_flow_context(flow_ctx)

        # Also create a task context for output capture
        task_ctx = Context(task_name=f"flow:{fn.__name__}")
        existing_task_ctx = get_context()

        if existing_task_ctx is None:
            set_context(task_ctx)

        try:
            result = fn(*args, **kwargs)

            # Ensure the result is a Result type
            if not isinstance(result, Result):
                result = Ok(result)

            # Attach flow context to the result for inspection
            result._flow_context = flow_ctx  # type: ignore[attr-defined]

            return result

        except Exception as e:
            tb = traceback.format_exc()
            err_result = Err(f"{type(e).__name__}: {e}", traceback=tb)
            err_result._flow_context = flow_ctx  # type: ignore[attr-defined]
            return err_result

        finally:
            set_flow_context(None)
            if existing_task_ctx is None:
                set_context(None)

    # Create flow info
    info = FlowInfo(
        name=fn.__name__,
        module=fn.__module__,
        fn=wrapper,
        original_fn=fn,
        signature=inspect.signature(fn),
        doc=fn.__doc__,
    )
    _flow_registry[info.full_name] = info

    # Attach flow info to wrapper
    wrapper._flow_info = info  # type: ignore[attr-defined]

    return wrapper
