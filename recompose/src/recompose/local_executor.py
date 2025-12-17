"""Local flow execution with subprocess isolation.

This module provides the local execution engine for flows, running each step
as a separate subprocess. This matches the behavior of GitHub Actions workflows
where each step is isolated.

The main entry point is `execute_flow_isolated()` which:
1. Builds a FlowPlan from the flow
2. Creates a workspace directory for inter-step communication
3. Runs each step as a subprocess via the CLI's --step mode
4. Renders progress with a tree-based display
"""

from __future__ import annotations

import inspect
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any

from rich.console import Console

from .conditional import evaluate_condition
from .context import dbg, get_entry_point, is_debug
from .expr import format_expr
from .plan import InputPlaceholder
from .result import Err, Ok, Result
from .workspace import FlowParams, create_workspace, read_step_result, write_params

if TYPE_CHECKING:
    from .flow import FlowWrapper


def _format_condition_expr(condition_data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    return format_expr(condition_data)


def execute_flow_isolated(
    flow: FlowWrapper,
    workspace: Path | None = None,
    **kwargs: Any,
) -> Result[None]:
    """
    Execute a flow with each step running as a separate subprocess.

    This is the local execution engine for recompose flows. It matches the behavior
    of GitHub Actions workflows where each step runs in isolation.

    Args:
        flow: The flow wrapper (decorated function with _flow_info and .plan())
        workspace: Optional workspace directory. If not provided, one is auto-generated.
        **kwargs: Flow parameters.

    Returns:
        Result[None] indicating success or failure of the flow.

    """
    flow_info = flow._flow_info
    flow_name = flow_info.name
    console = Console()

    # Build the plan with InputPlaceholders to preserve condition expressions
    plan_kwargs: dict[str, Any] = {}
    for param_name, param in flow_info.signature.parameters.items():
        annotation = param.annotation if param.annotation is not inspect.Parameter.empty else None
        default = param.default if param.default is not inspect.Parameter.empty else None
        plan_kwargs[param_name] = InputPlaceholder(name=param_name, annotation=annotation, default=default)

    plan = flow.plan(**plan_kwargs)

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
        entry_point = ("script", inspect.getfile(flow_info.original_fn))

    entry_type, entry_value = entry_point

    if is_debug():
        dbg(f"Flow: {flow_name}")
        dbg(f"Entry point: {entry_type} -> {entry_value}")
        dbg(f"Workspace: {ws}")
        dbg(f"Steps: {[s[0] for s in steps]}")
        dbg(f"Params: {kwargs}")

    # Write params (setup step)
    flow_params = FlowParams(
        flow_name=flow_name,
        params=kwargs,
        steps=[s[0] for s in steps],
        created_at=datetime.now().isoformat(),
        script_path=entry_value,  # Store the module name or script path
    )
    write_params(ws, flow_params)

    # Create the tree renderer
    from .output import FlowRenderer

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
