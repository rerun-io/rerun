"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
import time
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Any, ParamSpec, Protocol, TypeVar, cast

from .plan import FlowPlan
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


def _format_condition_expr(condition_data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    from .expr import format_expr

    return format_expr(condition_data)


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
        # Direct flow execution uses subprocess isolation (matches GHA behavior)
        return run_isolated_impl(**kwargs)

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

        from .conditional import evaluate_condition
        from .context import dbg, get_entry_point, is_debug
        from .output import FlowRenderer
        from .workspace import create_workspace, read_step_result, write_params

        flow_name = fn.__name__
        console = Console()

        # Build the plan with InputPlaceholders to preserve condition expressions
        from .plan import InputPlaceholder

        flow_sig = inspect.signature(fn)
        plan_kwargs: dict[str, Any] = {}
        for param_name, param in flow_sig.parameters.items():
            annotation = param.annotation if param.annotation is not inspect.Parameter.empty else None
            default = param.default if param.default is not inspect.Parameter.empty else None
            plan_kwargs[param_name] = InputPlaceholder(name=param_name, annotation=annotation, default=default)

        plan = plan_only(**plan_kwargs)

        # Use linear order from flow definition - no topological sort needed
        # Assign step names based on linear order
        plan.assign_step_names()

        # Get steps in linear order (skip GHA actions for local execution)
        steps = [(n.step_name or n.name, n) for n in plan.nodes if not n.task_info.is_gha_action]

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

            # Check if this step has a condition - evaluate inline
            condition_expr_str: str | None = None
            condition_value: bool | None = None
            if node.condition is not None:
                # Format the condition expression for display
                condition_expr_str = _format_condition_expr(node.condition.serialize())

                # Evaluate the condition with actual parameter values
                cond_result = evaluate_condition(node.condition.serialize(), kwargs, {})
                condition_value = cond_result.value() if cond_result.ok else False

                if not condition_value:
                    # Condition is false, skip this step
                    renderer.step_skipped_conditional(step_name, step_idx, condition_expr_str, condition_value)
                    continue

            # Print step header (with condition if present)
            renderer.step_header(step_name, step_idx, condition_expr=condition_expr_str)

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
