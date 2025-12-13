"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
import time
import traceback
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Any, ParamSpec, Protocol, TypeVar, cast

from .context import Context, get_context, set_context
from .flowgraph import FlowPlan, TaskNode
from .result import Err, Ok, Result

P = ParamSpec("P")
T = TypeVar("T")


class FlowWrapper(Protocol[T]):
    """
    Protocol describing a flow-decorated function.

    Flow wrappers are callable (returning Result[T]) and have a .plan() method
    for inspecting the task graph without execution.
    """

    _flow_info: FlowInfo

    def __call__(self, **kwargs: Any) -> Result[T]: ...

    def plan(self, **kwargs: Any) -> FlowPlan: ...


# Context variable for declarative flow plan building
_current_plan: ContextVar[FlowPlan | None] = ContextVar("recompose_current_plan", default=None)


def get_current_plan() -> FlowPlan | None:
    """Get the current flow plan being built, or None if not in a declarative flow."""
    return _current_plan.get()


def set_current_plan(plan: FlowPlan | None) -> None:
    """Set the current flow plan (used by @flow decorator)."""
    _current_plan.set(plan)


class TaskFailed(Exception):
    """
    Raised when a task fails inside a flow.

    This is used internally to short-circuit flow execution.
    The flow decorator catches this and returns the failed Result.
    """

    def __init__(self, result: Result[Any]):
        self.result = result
        super().__init__(result.error or "Task failed")


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


def _resolve_kwargs(kwargs: dict[str, Any], results: dict[str, Result[Any]]) -> dict[str, Any]:
    """Replace TaskNode values in kwargs with their actual results."""
    resolved = {}
    for k, v in kwargs.items():
        if isinstance(v, TaskNode):
            # Get the result for this node and unwrap it
            node_result = results.get(v.node_id)
            if node_result is None:
                raise RuntimeError(f"Dependency {v.name} ({v.node_id}) has not been executed yet")
            if node_result.failed:
                raise RuntimeError(f"Dependency {v.name} failed: {node_result.error}")
            resolved[k] = node_result.value
        else:
            resolved[k] = v
    return resolved


def _execute_plan(plan: FlowPlan, flow_ctx: FlowContext) -> Result[Any]:
    """Execute a declarative flow plan in topological order."""
    import time

    results: dict[str, Result[Any]] = {}

    for node in plan.get_execution_order():
        # Resolve any TaskNode dependencies in kwargs
        try:
            resolved_kwargs = _resolve_kwargs(node.kwargs, results)
        except RuntimeError as e:
            return Err(str(e))

        # Execute the task's original function (not the wrapper)
        # This avoids double-recording in flow context
        start_time = time.perf_counter()
        try:
            result = node.task_info.original_fn(**resolved_kwargs)
            # Ensure result is a Result type
            if not isinstance(result, Result):
                result = Ok(result)
        except Exception as e:
            tb = traceback.format_exc()
            result = Err(f"{type(e).__name__}: {e}", traceback=tb)

        duration = time.perf_counter() - start_time

        # Record in flow context
        flow_ctx.record_task(node.task_info.name, result, duration)

        # Store result by node_id
        results[node.node_id] = result

        # Fail-fast if task failed
        if result.failed:
            return result

    # Return the terminal node's result
    if plan.terminal and plan.terminal.node_id in results:
        return results[plan.terminal.node_id]
    elif plan.nodes:
        # No explicit terminal - return last node's result
        return results[plan.nodes[-1].node_id]
    else:
        return Ok(None)


class DirectTaskCallInFlowError(Exception):
    """Raised when a task is called directly (not via .flow()) inside a flow."""

    def __init__(self, task_name: str):
        super().__init__(
            f"Task '{task_name}' was called directly inside a flow. "
            f"Use '{task_name}.flow(...)' instead to build the task graph."
        )


def flow(fn: Callable[..., TaskNode[T]]) -> FlowWrapper[T]:
    """
    Decorator to mark a function as a recompose flow.

    A flow composes tasks into a dependency graph using task.flow() calls.
    The flow function must return a TaskNode (the terminal node of the graph).

    Example:
        @recompose.flow
        def build_pipeline(*, repo: str):
            source = fetch_source.flow(repo=repo)
            binary = compile.flow(source=source)
            tested = test.flow(binary=binary)
            return tested  # Returns TaskNode - the terminal node

        # Execute the flow
        result = build_pipeline(repo="main")

        # Or inspect the plan first
        plan = build_pipeline.plan(repo="main")

    The flow wrapper provides:
    - Direct call: Builds the graph and executes it
    - flow.plan(**kwargs): Build the plan without executing (for dry-run)
    """

    @functools.wraps(fn)
    def wrapper(**kwargs: Any) -> Result[T]:
        # Create flow context for tracking executions
        flow_ctx = FlowContext(flow_name=fn.__name__)
        set_flow_context(flow_ctx)

        # Create a plan context - .flow() calls will register nodes here
        plan = FlowPlan()
        set_current_plan(plan)

        # Create a task context for output capture during execution
        task_ctx = Context(task_name=f"flow:{fn.__name__}")
        existing_task_ctx = get_context()

        if existing_task_ctx is None:
            set_context(task_ctx)

        try:
            # Run the flow function body to build the task graph
            flow_return = fn(**kwargs)

            # Flow must return a TaskNode
            if not isinstance(flow_return, TaskNode):
                raise TypeError(
                    f"Flow '{fn.__name__}' must return a TaskNode, "
                    f"got {type(flow_return).__name__}. "
                    "Use task.flow() calls and return the terminal TaskNode."
                )

            # Set the terminal node and execute the plan
            plan.terminal = flow_return
            set_current_plan(None)  # Clear before execution

            result = _execute_plan(plan, flow_ctx)
            result._flow_context = flow_ctx  # type: ignore[attr-defined]
            result._flow_plan = plan  # type: ignore[attr-defined]
            return result

        except Exception as e:
            if isinstance(e, (TypeError, DirectTaskCallInFlowError)):
                raise  # Re-raise flow construction errors
            tb = traceback.format_exc()
            err_result: Result[T] = Err(f"{type(e).__name__}: {e}", traceback=tb)
            err_result._flow_context = flow_ctx  # type: ignore[attr-defined]
            return err_result

        finally:
            set_flow_context(None)
            set_current_plan(None)
            if existing_task_ctx is None:
                set_context(None)

    def plan_only(**kwargs: Any) -> FlowPlan:
        """
        Build the flow plan without executing it.

        This runs the flow function body to build the task graph,
        but does not execute any tasks. Useful for dry-run and visualization.

        Returns:
            FlowPlan with all TaskNodes and their dependencies.
        """
        plan = FlowPlan()
        set_current_plan(plan)

        try:
            flow_return = fn(**kwargs)

            if not isinstance(flow_return, TaskNode):
                raise TypeError(
                    f"Flow '{fn.__name__}' must return a TaskNode, "
                    f"got {type(flow_return).__name__}. "
                    "Use task.flow() calls and return the terminal TaskNode."
                )

            plan.terminal = flow_return
            return plan
        finally:
            set_current_plan(None)

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

    # Attach flow info and plan method to wrapper
    wrapper._flow_info = info  # type: ignore[attr-defined]
    wrapper.plan = plan_only  # type: ignore[attr-defined]

    # Cast to FlowWrapper to satisfy type checker
    return cast(FlowWrapper[T], wrapper)
