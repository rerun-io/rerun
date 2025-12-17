"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Any, ParamSpec, Protocol, TypeVar, cast

from .plan import FlowPlan
from .result import Result

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

        This delegates to local_executor.execute_flow_isolated().
        """
        from .local_executor import execute_flow_isolated

        return execute_flow_isolated(wrapper, workspace=workspace, **kwargs)

    @functools.wraps(fn)
    def wrapper(**kwargs: Any) -> Result[None]:
        # Direct flow execution uses subprocess isolation (matches GHA behavior)
        return run_isolated_impl(**kwargs)

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
