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
            resolved[k] = node_result.value()
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


def _format_condition_expr(condition_data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    expr_type = condition_data.get("type", "")
    if expr_type == "input":
        return str(condition_data.get("name", "?"))
    elif expr_type == "literal":
        return str(condition_data.get("value", "?"))
    elif expr_type == "binary":
        left = _format_condition_expr(condition_data.get("left", {}))
        op = condition_data.get("op", "?")
        right = _format_condition_expr(condition_data.get("right", {}))
        return f"{left} {op} {right}"
    elif expr_type == "unary":
        op = condition_data.get("op", "?")
        operand = _format_condition_expr(condition_data.get("operand", {}))
        return f"{op} {operand}"
    return "?"


def flow(fn: Callable[..., None]) -> FlowWrapper:
    """
    Decorator to mark a function as a recompose flow.

    A flow composes tasks into a dependency graph using task calls.
    Tasks automatically detect they're in a flow-building context and
    return TaskNodes instead of executing. The last task call becomes
    the terminal node of the graph.

    Example:
        @recompose.flow
        def build_pipeline(*, repo: str) -> None:
            source = fetch_source(repo=repo)
            binary = compile(source=source)
            test(binary=binary)  # Last call is the terminal

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
                raise ValueError(f"Flow '{fn.__name__}' has no tasks. Use task calls to add tasks.")
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
            if isinstance(e, (ValueError, TypeError)):
                raise  # Re-raise flow construction errors (programming mistakes)
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
                raise ValueError(f"Flow '{fn.__name__}' has no tasks. Use task calls to add tasks.")
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
        import os
        import subprocess
        import sys

        from rich.console import Console

        from .context import dbg, get_entry_point, is_debug
        from .output import FlowRenderer
        from .workspace import create_workspace, read_step_result, write_params

        flow_name = fn.__name__
        console = Console()

        # Build the plan to get step names
        plan = plan_only(**kwargs)

        # Inject condition-check nodes if there are conditional tasks
        from .gha import _create_eval_condition_task_info

        condition_task_info = _create_eval_condition_task_info()
        plan.inject_condition_checks(condition_task_info)

        # Step names are now assigned by inject_condition_checks
        steps = plan.get_steps()

        # Create or use provided workspace
        ws = create_workspace(flow_name, workspace=workspace)

        # Get entry point info - use the same invocation method as the parent
        entry_point = get_entry_point()
        if entry_point is None:
            # Fallback to script mode with the module where the flow is defined
            entry_point = ("script", inspect.getfile(fn))

        entry_type, entry_value = entry_point

        if is_debug():
            dbg(f"Flow: {flow_name}")
            dbg(f"Entry point: {entry_type} -> {entry_value}")
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
            script_path=entry_value,  # Store the module name or script path
        )
        write_params(ws, flow_params)

        # Create the tree renderer
        renderer = FlowRenderer(console, flow_name, len(steps))
        renderer.start()

        flow_start_time = time.perf_counter()
        failed_step: str | None = None
        failed_error: str | None = None

        # Execute each step as a subprocess
        for step_idx, (step_name, node) in enumerate(steps, start=1):
            # If a previous step failed, skip remaining steps
            if failed_step is not None:
                renderer.step_skipped(step_name, step_idx, f"prior failure in {failed_step}")
                continue

            # Check if this step has a condition that needs to be evaluated
            if node.condition_check_step:
                # Read the condition result from workspace
                cond_result = read_step_result(ws, node.condition_check_step)
                if cond_result.ok and cond_result.value() is False:
                    # Condition is false, skip this step
                    renderer.step_skipped(step_name, step_idx, "condition false")
                    continue

            # Check if this is a condition evaluation step
            is_condition_step = node.task_info.is_condition_check

            # Print step header (condition steps handled differently after execution)
            if not is_condition_step:
                renderer.step_header(step_name, step_idx)

            # Build command based on entry point type
            if entry_type == "module":
                cmd = [sys.executable, "-m", entry_value]
            else:
                cmd = [sys.executable, entry_value]

            cmd.extend(
                [
                    flow_name,
                    "--step",
                    step_name,
                    "--workspace",
                    str(ws),
                ]
            )

            if is_debug():
                dbg(f"Running: {' '.join(cmd)}")

            # Set up environment with tree rendering context
            step_env = os.environ.copy()
            step_env.update(renderer.get_step_env(step_idx))

            # Run step as subprocess (output streams directly with tree prefix)
            step_start = time.perf_counter()
            result = subprocess.run(cmd, capture_output=False, env=step_env)
            step_duration = time.perf_counter() - step_start

            # Read the result from workspace
            step_result = read_step_result(ws, step_name)
            result_value = step_result.value() if step_result.ok else None

            if result.returncode != 0:
                # Step failed - record failure but continue to show remaining steps as skipped
                error_msg = step_result.error if step_result.failed else f"exit code {result.returncode}"
                renderer.step_failed(step_name, step_idx, step_duration, error_msg)
                failed_step = step_name
                failed_error = step_result.error or f"Step {step_name} failed"
                continue

            # Step succeeded
            if is_condition_step:
                # Get the condition expression for display
                condition_data = node.kwargs.get("condition_data", {})
                condition_expr = _format_condition_expr(condition_data)
                renderer.step_condition(step_name, step_idx, condition_expr, bool(result_value), step_duration)
            else:
                renderer.step_success(step_name, step_idx, step_duration, result_value)

        # Finish with appropriate status
        if failed_step is not None:
            renderer.finish(success=False, duration=time.perf_counter() - flow_start_time)
            return Err(failed_error or f"Step {failed_step} failed")

        renderer.finish(success=True, duration=time.perf_counter() - flow_start_time)
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
            raise RuntimeError(f"{info.name}.dispatch() can only be called inside an @automation-decorated function.")

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
