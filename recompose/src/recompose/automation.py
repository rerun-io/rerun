"""Automation decorator for orchestrating flows.

Automations are higher-level compositions that orchestrate multiple flows.
They generate GitHub Actions workflows that use workflow_dispatch to trigger
child flows and workflow_run to chain executions.
"""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import Any, TypeVar

T = TypeVar("T")


@dataclass
class FlowDispatch:
    """
    Represents a dispatched flow within an automation.

    When you call `my_flow.dispatch(...)` inside an automation, it creates
    a FlowDispatch that records:
    - Which flow to dispatch
    - What parameters to pass
    - Any GHA overrides (like runs_on)
    """

    flow_name: str
    params: dict[str, Any] = field(default_factory=dict)
    runs_on: str | None = None  # Override the default runner

    def __repr__(self) -> str:
        params_str = ", ".join(f"{k}={v!r}" for k, v in self.params.items())
        return f"FlowDispatch({self.flow_name}, {params_str})"


@dataclass
class AutomationPlan:
    """
    The execution plan for an automation.

    Tracks all flow dispatches made during automation construction.
    """

    dispatches: list[FlowDispatch] = field(default_factory=list)

    def add_dispatch(self, dispatch: FlowDispatch) -> None:
        """Record a flow dispatch."""
        self.dispatches.append(dispatch)


# Context variable for the current automation plan
_current_automation_plan: ContextVar[AutomationPlan | None] = ContextVar("recompose_automation_plan", default=None)


def get_current_automation_plan() -> AutomationPlan | None:
    """Get the current automation plan being built, or None."""
    return _current_automation_plan.get()


def set_current_automation_plan(plan: AutomationPlan | None) -> None:
    """Set the current automation plan."""
    _current_automation_plan.set(plan)


@dataclass
class AutomationInfo:
    """Metadata about a registered automation."""

    name: str
    module: str
    fn: Callable[..., None]
    original_fn: Callable[..., None]
    signature: inspect.Signature
    doc: str | None

    # GHA configuration
    gha_on: dict[str, Any] | None = None
    gha_runs_on: str = "ubuntu-latest"
    gha_env: dict[str, str] | None = None
    gha_timeout_minutes: int | None = None

    @property
    def full_name(self) -> str:
        """Full qualified name of the automation."""
        return f"{self.module}:{self.name}"


# Global registry of all automations
_automation_registry: dict[str, AutomationInfo] = {}


def get_automation_registry() -> dict[str, AutomationInfo]:
    """Get the automation registry."""
    return _automation_registry


def get_automation(name: str) -> AutomationInfo | None:
    """Get an automation by name."""
    if name in _automation_registry:
        return _automation_registry[name]

    for full_name, info in _automation_registry.items():
        if info.name == name:
            return info

    return None


def automation(
    fn: Callable[..., None] | None = None,
    *,
    gha_on: dict[str, Any] | None = None,
    gha_runs_on: str = "ubuntu-latest",
    gha_env: dict[str, str] | None = None,
    gha_timeout_minutes: int | None = None,
) -> Callable[..., None] | Callable[[Callable[..., None]], Callable[..., None]]:
    """
    Decorator to mark a function as a recompose automation.

    Automations orchestrate multiple flows via dispatch. They generate
    GitHub Actions workflows that trigger child flows via workflow_dispatch.

    Example:
        @recompose.automation(
            gha_on={"schedule": [{"cron": "0 0 * * *"}]},
            gha_runs_on="ubuntu-latest",
        )
        def nightly_build():
            build_pipeline.dispatch(repo="main")
            run_tests.dispatch()

    Args:
        gha_on: GitHub Actions trigger configuration (schedule, push, etc.)
        gha_runs_on: Runner for the orchestration job
        gha_env: Environment variables for the job
        gha_timeout_minutes: Job timeout

    The automation can then generate a workflow YAML via:
        ./app.py generate-gha nightly_build

    """

    def decorator(func: Callable[..., None]) -> Callable[..., None]:
        @functools.wraps(func)
        def wrapper(**kwargs: Any) -> None:
            # Build the automation plan
            plan = AutomationPlan()
            set_current_automation_plan(plan)

            try:
                func(**kwargs)
            finally:
                set_current_automation_plan(None)

        # Create automation info
        info = AutomationInfo(
            name=func.__name__,
            module=func.__module__,
            fn=wrapper,
            original_fn=func,
            signature=inspect.signature(func),
            doc=func.__doc__,
            gha_on=gha_on,
            gha_runs_on=gha_runs_on,
            gha_env=gha_env,
            gha_timeout_minutes=gha_timeout_minutes,
        )
        _automation_registry[info.full_name] = info

        # Attach info and plan method
        wrapper._automation_info = info  # type: ignore[attr-defined]

        def plan_only(**kwargs: Any) -> AutomationPlan:
            """Build the automation plan without executing dispatches."""
            plan = AutomationPlan()
            set_current_automation_plan(plan)
            try:
                func(**kwargs)
                return plan
            finally:
                set_current_automation_plan(None)

        wrapper.plan = plan_only  # type: ignore[attr-defined]

        return wrapper

    # Handle both @automation and @automation(...) syntax
    if fn is not None:
        return decorator(fn)
    return decorator
