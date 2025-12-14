"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
import time
import traceback
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, ParamSpec, Protocol, TypeVar, cast

from .context import Context, get_context, set_context
from .flowgraph import FlowPlan, TaskNode
from .result import Err, Ok, Result

P = ParamSpec("P")
T = TypeVar("T")


class FlowWrapper(Protocol):
    """
    Protocol describing a flow-decorated function.

    Flow wrappers are callable (returning Result[None]) and have:
    - .plan(): Inspect the task graph without execution
    - .run_isolated(): Execute each step as a separate subprocess
    - .dispatch(): Trigger this flow from within an automation
    """

    _flow_info: FlowInfo

    def __call__(self, **kwargs: Any) -> Result[None]: ...

    def plan(self, **kwargs: Any) -> FlowPlan: ...

    def run_isolated(self, **kwargs: Any) -> Result[None]: ...

    def dispatch(self, **kwargs: Any) -> Any: ...


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
        result: Result[Any]
        try:
            task_return = node.task_info.original_fn(**resolved_kwargs)
            # Ensure result is a Result type
            if isinstance(task_return, Result):
                result = task_return
            else:
                result = Ok(task_return)
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


def flow(fn: Callable[..., None]) -> FlowWrapper:
    """
    Decorator to mark a function as a recompose flow.

    A flow composes tasks into a dependency graph using task.flow() calls.
    The last task.flow() call becomes the terminal node of the graph.

    Example:
        @recompose.flow
        def build_pipeline(*, repo: str) -> None:
            source = fetch_source.flow(repo=repo)
            binary = compile.flow(source=source)
            test.flow(binary=binary)  # Last call is the terminal

        # Execute the flow
        result = build_pipeline(repo="main")

        # Or inspect the plan first
        plan = build_pipeline.plan(repo="main")

    The flow wrapper provides:
    - Direct call: Builds the graph and executes it
    - flow.plan(**kwargs): Build the plan without executing (for dry-run)
    """

    @functools.wraps(fn)
    def wrapper(**kwargs: Any) -> Result[None]:
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
            fn(**kwargs)

            # Use the last added node as the terminal
            if not plan.nodes:
                raise ValueError(f"Flow '{fn.__name__}' has no tasks. Use task.flow() calls to add tasks.")
            plan.terminal = plan.nodes[-1]
            set_current_plan(None)  # Clear before execution

            exec_result = _execute_plan(plan, flow_ctx)

            # If a task failed, propagate that failure
            if exec_result.failed:
                result: Result[None] = Err(exec_result.error or "Task failed", traceback=exec_result.traceback)
            else:
                result = Ok(None)

            result._flow_context = flow_ctx  # type: ignore[attr-defined]
            result._flow_plan = plan  # type: ignore[attr-defined]
            return result

        except Exception as e:
            if isinstance(e, (DirectTaskCallInFlowError, ValueError)):
                raise  # Re-raise flow construction errors
            tb = traceback.format_exc()
            err_result: Result[None] = Err(f"{type(e).__name__}: {e}", traceback=tb)
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
            fn(**kwargs)

            if not plan.nodes:
                raise ValueError(f"Flow '{fn.__name__}' has no tasks. Use task.flow() calls to add tasks.")
            plan.terminal = plan.nodes[-1]
            return plan
        finally:
            set_current_plan(None)

    def run_isolated_impl(workspace: Path | None = None, **kwargs: Any) -> Result[None]:
        """
        Execute the flow with each step running as a separate subprocess.

        This is useful for:
        - Testing subprocess isolation locally
        - Debugging step-by-step execution
        - Matching the behavior of generated GitHub Actions workflows

        Args:
            workspace: Optional workspace directory. If not provided, one is auto-generated.
            **kwargs: Flow parameters.

        Returns:
            Result[None] indicating success or failure of the flow.
        """
        import subprocess
        import sys

        from .context import dbg, is_debug
        from .workspace import create_workspace, read_step_result, write_params

        flow_name = fn.__name__

        # Build the plan to get step names
        plan = plan_only(**kwargs)
        plan.assign_step_names()
        steps = plan.get_steps()

        # Create or use provided workspace
        ws = create_workspace(flow_name, workspace=workspace)

        # Get script path - use the module where the flow is defined
        script_path = inspect.getfile(fn)

        if is_debug():
            dbg(f"Flow: {flow_name}")
            dbg(f"Script: {script_path}")
            dbg(f"Workspace: {ws}")
            dbg(f"Steps: {[s[0] for s in steps]}")
            dbg(f"Params: {kwargs}")

        # Write params (setup step)
        from datetime import datetime

        from .workspace import FlowParams

        flow_params = FlowParams(
            flow_name=flow_name,
            params=kwargs,
            steps=[s[0] for s in steps],
            created_at=datetime.now().isoformat(),
            script_path=script_path,
        )
        write_params(ws, flow_params)

        # Execute each step as a subprocess
        for step_name, _node in steps:
            cmd = [
                sys.executable,
                script_path,
                flow_name,
                "--step",
                step_name,
                "--workspace",
                str(ws),
            ]

            if is_debug():
                dbg(f"Running: {' '.join(cmd)}")

            result = subprocess.run(cmd, capture_output=False)

            if result.returncode != 0:
                # Step failed - read its result if available
                step_result = read_step_result(ws, step_name)
                if step_result.failed:
                    return Err(step_result.error or f"Step {step_name} failed")
                return Err(f"Step {step_name} failed with exit code {result.returncode}")

        # All steps succeeded
        return Ok(None)

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

    def dispatch_impl(runs_on: str | None = None, **kwargs: Any) -> Any:
        """
        Dispatch this flow from within an automation.

        This method can only be called inside an @automation-decorated function.
        It records the dispatch in the automation plan.

        Args:
            runs_on: Optional runner override for this specific dispatch
            **kwargs: Flow parameters to pass when dispatching

        Returns:
            FlowDispatch handle representing the dispatched workflow
        """
        from .automation import FlowDispatch, get_current_automation_plan

        plan = get_current_automation_plan()
        if plan is None:
            raise RuntimeError(
                f"{info.name}.dispatch() can only be called inside an @automation-decorated function."
            )

        dispatch = FlowDispatch(
            flow_name=info.name,
            params=kwargs,
            runs_on=runs_on,
        )
        plan.add_dispatch(dispatch)
        return dispatch

    # Attach flow info, plan method, run_isolated, and dispatch to wrapper
    wrapper._flow_info = info  # type: ignore[attr-defined]
    wrapper.plan = plan_only  # type: ignore[attr-defined]
    wrapper.run_isolated = run_isolated_impl  # type: ignore[attr-defined]
    wrapper.dispatch = dispatch_impl  # type: ignore[attr-defined]

    # Cast to FlowWrapper to satisfy type checker
    return cast(FlowWrapper, wrapper)
