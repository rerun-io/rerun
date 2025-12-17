"""Flow decorator for composing tasks."""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Any, ParamSpec, Protocol, TypeVar, cast

from .plan import FlowPlan, InputPlaceholder
from .result import Result

P = ParamSpec("P")
T = TypeVar("T")


class FlowWrapper(Protocol):
    """
    Protocol describing a flow-decorated function.

    Flow wrappers are callable (returning Result[None]) and have:
    - ._flow_info: Metadata about the flow
    - .plan: The pre-built FlowPlan (computed at decoration time)
    - .dispatch(): Trigger this flow from within an automation
    """

    _flow_info: FlowInfo

    def __call__(self, **kwargs: Any) -> Result[None]: ...

    @property
    def plan(self) -> FlowPlan: ...

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
    plan: FlowPlan  # The pre-built plan (computed at decoration time)

    @property
    def full_name(self) -> str:
        """Full qualified name of the flow."""
        return f"{self.module}:{self.name}"


def _build_plan(fn: Callable[..., None]) -> FlowPlan:
    """
    Build the flow plan at decoration time.

    Executes the flow function body with InputPlaceholders for all parameters.
    This builds the task graph without executing any tasks.

    If the flow uses parameters in Python control flow (if statements, loops),
    InputPlaceholder.__bool__ will raise a clear error explaining how to use
    run_if() for conditional execution instead.

    Raises:
        ValueError: If the flow has no tasks
        TypeError: If flow parameters are used in Python control flow

    """
    sig = inspect.signature(fn)

    # Create InputPlaceholders for all parameters
    plan_kwargs: dict[str, Any] = {}
    for param_name, param in sig.parameters.items():
        annotation = param.annotation if param.annotation is not inspect.Parameter.empty else None
        default = param.default if param.default is not inspect.Parameter.empty else None
        plan_kwargs[param_name] = InputPlaceholder(name=param_name, annotation=annotation, default=default)

    # Build the plan
    plan = FlowPlan()
    set_current_plan(plan)

    try:
        fn(**plan_kwargs)

        if not plan.nodes:
            raise ValueError(f"Flow '{fn.__name__}' has no tasks. Use task calls to add tasks.")
        plan.terminal = plan.nodes[-1]
        plan.assign_step_names()
        return plan
    finally:
        set_current_plan(None)


def flow(fn: Callable[..., None]) -> FlowWrapper:
    """
    Decorator to mark a function as a recompose flow.

    A flow composes tasks into a dependency graph using task calls.
    The plan is built eagerly at decoration time - if there are any errors
    in the flow structure (e.g., using parameters in control flow), they
    are raised immediately.

    Example:
        @recompose.flow
        def build_pipeline(*, repo: str) -> None:
            source = fetch_source(repo=repo)
            binary = compile(source=source)
            test(binary=binary)  # Last call is the terminal

        # Execute the flow
        result = build_pipeline(repo="main")

        # Or inspect the pre-built plan
        plan = build_pipeline.plan

    The flow wrapper provides:
    - Direct call: Executes the flow with subprocess isolation
    - .plan: The pre-built FlowPlan (read-only property)
    - .dispatch(): Trigger from within an automation

    """
    # Build the plan eagerly at decoration time
    # This catches errors like using parameters in control flow immediately
    built_plan = _build_plan(fn)

    @functools.wraps(fn)
    def wrapper(**kwargs: Any) -> Result[None]:
        # Direct flow execution uses subprocess isolation (matches GHA behavior)
        from .local_executor import execute_flow_isolated

        return execute_flow_isolated(wrapper, **kwargs)  # type: ignore[arg-type]

    # Create flow info with the pre-built plan
    info = FlowInfo(
        name=fn.__name__,
        module=fn.__module__,
        fn=wrapper,
        original_fn=fn,
        signature=inspect.signature(fn),
        doc=fn.__doc__,
        plan=built_plan,
    )

    # Attach flow info and plan to wrapper
    wrapper._flow_info = info  # type: ignore[attr-defined]
    wrapper.plan = built_plan  # type: ignore[attr-defined]

    # dispatch() is implemented in automation.py to avoid circular dependency
    # It's attached here as a bound method
    from .automation import create_dispatch_method

    wrapper.dispatch = create_dispatch_method(info)  # type: ignore[attr-defined]

    # Cast to FlowWrapper to satisfy type checker
    return cast(FlowWrapper, wrapper)
