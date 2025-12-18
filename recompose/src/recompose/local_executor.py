"""Local flow execution with subprocess isolation.

This module provides the local execution engine for flows, running each step
as a separate subprocess. This matches the behavior of GitHub Actions workflows
where each step is isolated.

The main entry points are:
- `execute_flow_isolated()`: Runs a complete flow locally with subprocess isolation
- `setup_workspace()`: Initializes a workspace for a flow (used by GHA setup step)
- `run_step()`: Executes a single step (used by both local executor and GHA)
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
from .flow import FlowInfo
from .result import Err, Ok, Result
from .workspace import (
    FlowParams,
    create_workspace,
    read_params,
    read_step_result,
    read_taskclass_state,
    write_params,
    write_step_result,
    write_taskclass_state,
)

if TYPE_CHECKING:
    from .flow import FlowWrapper


def _format_condition_expr(condition_data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    return format_expr(condition_data)


# =============================================================================
# Setup Workspace
# =============================================================================


def setup_workspace(
    flow_info: FlowInfo,
    workspace: Path | None = None,
    **kwargs: Any,
) -> Path:
    """
    Initialize a workspace for a flow.

    This creates the workspace directory and writes the flow parameters.
    Used by GHA workflows in the setup step before running individual steps.

    Args:
        flow_info: The flow's FlowInfo metadata
        workspace: Optional workspace directory. If not provided, one is auto-generated.
        **kwargs: Flow parameters to store in the workspace.

    Returns:
        Path to the workspace directory.

    """
    flow_name = flow_info.name

    # Create or use provided workspace
    ws = create_workspace(flow_name, workspace=workspace)

    # Use the pre-built plan (step names already assigned at decoration time)
    plan = flow_info.plan
    step_names = [n.step_name for n in plan.nodes if n.step_name]

    # Get entry point info
    entry_point = get_entry_point()
    script_path = entry_point[1] if entry_point else sys.argv[0]

    flow_params = FlowParams(
        flow_name=flow_name,
        params=kwargs,
        steps=step_names,
        created_at=datetime.now().isoformat(),
        script_path=script_path,
    )
    write_params(ws, flow_params)

    return ws


# =============================================================================
# Run Step
# =============================================================================


def run_step(
    flow_info: FlowInfo,
    step: str,
    workspace: Path,
) -> Result[Any]:
    """
    Execute a single step of a flow.

    This resolves dependencies from the workspace, executes the task, and
    writes the result back to the workspace. Used by both local executor
    (via subprocess) and GHA workflows.

    Args:
        flow_info: The flow's FlowInfo metadata
        step: The step name to execute
        workspace: The workspace directory containing flow params and step results

    Returns:
        Result containing the step's return value or error.

    """
    console = Console()

    # Read params from workspace
    try:
        flow_params = read_params(workspace)
    except FileNotFoundError:
        return Err(f"No _params.json in {workspace}. Run setup first.")

    # Use the pre-built plan
    plan = flow_info.plan

    # Find the requested step
    target_node = plan.get_step(step)
    if target_node is None:
        return Err(f"Step '{step}' not found. Available: {flow_params.steps}")

    step_name = target_node.step_name or target_node.name

    # Check if we're in tree mode (subprocess of run_isolated)
    from .output import install_tree_output, is_tree_mode, uninstall_tree_output

    tree_mode = is_tree_mode()

    if not tree_mode:
        console.print(f"\n[bold cyan]▶[/bold cyan] [bold]{step_name}[/bold]")
        console.print()

    # Install tree output wrapper for print/logging
    tree_ctx = install_tree_output()

    # Resolve dependencies from workspace
    from .plan import InputPlaceholder, TaskClassNode
    from .plan import TaskNode as TaskNodeType

    resolved_kwargs: dict[str, Any] = {}
    taskclass_node_proxy: Any = None  # Track if this is a TaskClass method call
    taskclass_id: str | None = None

    for kwarg_name, kwarg_value in target_node.kwargs.items():
        # Skip internal keys
        if kwarg_name == "__taskclass_node__":
            # This is a TaskClass method call - extract the TaskClassNode for state management
            if hasattr(kwarg_value, "node"):
                taskclass_node_proxy = kwarg_value.node  # Get TaskClassNode from proxy
            else:
                taskclass_node_proxy = kwarg_value
            taskclass_id = taskclass_node_proxy.node_id
            continue

        if kwarg_name == "__taskclass_id__":
            # Skip this - it's just for identifying the TaskClass, not for the function
            continue

        if isinstance(kwarg_value, TaskNodeType):  # TaskNode dependency
            dep_node = kwarg_value
            dep_step_name = dep_node.step_name or dep_node.name
            dep_result = read_step_result(workspace, dep_step_name)
            if dep_result.failed:
                uninstall_tree_output(tree_ctx)
                return Err(f"Dependency '{dep_step_name}' failed or not found")
            resolved_kwargs[kwarg_name] = dep_result.value()
        elif isinstance(kwarg_value, TaskClassNode):
            # TaskClass passed as parameter - deserialize from workspace
            tcn_id = kwarg_value.node_id
            instance = read_taskclass_state(workspace, tcn_id)
            if instance is None:
                uninstall_tree_output(tree_ctx)
                return Err(f"TaskClass state not found for {tcn_id}")
            resolved_kwargs[kwarg_name] = instance
        elif hasattr(kwarg_value, "_is_taskclass_node_proxy") and kwarg_value._is_taskclass_node_proxy:
            # TaskClassNodeProxy passed as parameter - get node and deserialize
            tcn = kwarg_value.node
            tcn_id = tcn.node_id
            instance = read_taskclass_state(workspace, tcn_id)
            if instance is None:
                uninstall_tree_output(tree_ctx)
                return Err(f"TaskClass state not found for {tcn_id}")
            resolved_kwargs[kwarg_name] = instance
        elif isinstance(kwarg_value, InputPlaceholder):
            # Resolve InputPlaceholder from flow params
            param_name = kwarg_value.name
            if param_name in flow_params.params:
                resolved_kwargs[kwarg_name] = flow_params.params[param_name]
            elif kwarg_value.default is not None:
                resolved_kwargs[kwarg_name] = kwarg_value.default
            else:
                uninstall_tree_output(tree_ctx)
                return Err(f"Required parameter '{param_name}' not found in workspace")
        else:
            resolved_kwargs[kwarg_name] = kwarg_value

    # Execute the task (or condition check)
    start_time = time.perf_counter()

    task_info = target_node.task_info
    taskclass_instance: Any = None  # Track instance for state serialization

    if task_info.is_condition_check:
        # Special handling for condition evaluation
        condition_data = target_node.kwargs.get("condition_data", {})

        # Build evaluation context: inputs from flow params, outputs from workspace
        eval_context_inputs = flow_params.params
        eval_context_outputs: dict[str, Any] = {}

        # Read outputs from previous steps that the condition might reference
        for prev_step in flow_params.steps:
            if prev_step == step_name:
                break  # Stop at current step
            prev_result = read_step_result(workspace, prev_step)
            if prev_result.ok:
                eval_context_outputs[prev_step] = prev_result.value()

        eval_result = evaluate_condition(condition_data, eval_context_inputs, eval_context_outputs)
        condition_value = eval_result.value() if eval_result.ok else False

        # Create a proper Result for workspace storage
        result = Ok(condition_value)

        # Write to GITHUB_OUTPUT if available (for GHA)
        github_output = os.environ.get("GITHUB_OUTPUT")
        if github_output:
            with open(github_output, "a") as f:
                f.write(f"value={'true' if condition_value else 'false'}\n")

    elif task_info.method_name == "__init__" and task_info.cls is not None:
        # TaskClass __init__ step - construct the instance
        cls = task_info.cls

        # Get the TaskClass ID from kwargs (stored during plan building)
        taskclass_id = target_node.kwargs.get("__taskclass_id__")
        if taskclass_id is None:
            uninstall_tree_output(tree_ctx)
            return Err("TaskClass __init__ missing __taskclass_id__ in kwargs")

        try:
            # Create instance - bypass our modified __new__ which returns a proxy in flow context
            # Use object.__new__ directly and then call original __init__
            taskclass_instance = object.__new__(cls)

            # Get the original __init__ (before any wrapping)
            original_init = task_info.original_fn
            original_init(taskclass_instance, **resolved_kwargs)

            result = Ok(None)  # __init__ returns None

        except Exception as e:
            import traceback

            tb = traceback.format_exc()
            result = Err(f"{type(e).__name__}: {e}", traceback=tb)

    elif taskclass_id is not None and task_info.is_method and task_info.method_name != "__init__":
        # TaskClass method step - deserialize instance, call method, serialize back
        taskclass_instance = read_taskclass_state(workspace, taskclass_id)
        if taskclass_instance is None:
            uninstall_tree_output(tree_ctx)
            return Err(f"TaskClass state not found for {taskclass_id}")

        # Get the bound method
        method_name = task_info.method_name
        if method_name is None:
            uninstall_tree_output(tree_ctx)
            return Err(f"TaskInfo missing method_name for TaskClass method")

        bound_method = getattr(taskclass_instance, method_name)

        # Execute with context management
        from .context import Context, get_context, set_context
        from .task import _execute_task

        existing_ctx = get_context()
        if existing_ctx is None:
            ctx = Context(task_name=step_name)
            set_context(ctx)
            try:
                result = _execute_task(bound_method, (), resolved_kwargs)
            finally:
                set_context(None)
        else:
            result = _execute_task(bound_method, (), resolved_kwargs)

    else:
        # Regular task - use the wrapped function (fn) which catches exceptions
        result = task_info.fn(**resolved_kwargs)

    elapsed = time.perf_counter() - start_time

    # Uninstall tree output wrapper
    uninstall_tree_output(tree_ctx)

    # Write TaskClass state if applicable
    if taskclass_instance is not None and taskclass_id is not None and result.ok:
        write_taskclass_state(workspace, taskclass_id, taskclass_instance)

    # Write result to workspace
    write_step_result(workspace, step_name, result)

    # Write value to GITHUB_OUTPUT if available (for non-condition steps too)
    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output and result.ok and result._value is not None:
        with open(github_output, "a") as f:
            f.write(f"value={result._value}\n")

    # Print result (only in non-tree mode - orchestrator handles tree formatting)
    if not tree_mode:
        if result.ok:
            console.print(f"[bold green]✓[/bold green] [bold]{step_name}[/bold] succeeded in {elapsed:.2f}s")
            if result._value is not None:
                console.print(f"[dim]→[/dim] {result._value}")
        else:
            console.print(f"[bold red]✗[/bold red] [bold]{step_name}[/bold] failed in {elapsed:.2f}s")
            if result.error:
                console.print(f"[red]Error:[/red] {result.error}")
        console.print()

    return result


# =============================================================================
# Execute Flow (Orchestrator)
# =============================================================================


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
        flow: The flow wrapper (decorated function with _flow_info and pre-built plan)
        workspace: Optional workspace directory. If not provided, one is auto-generated.
        **kwargs: Flow parameters.

    Returns:
        Result[None] indicating success or failure of the flow.

    """
    flow_info = flow._flow_info
    flow_name = flow_info.name
    console = Console()

    # Use the pre-built plan from the flow (built at decoration time)
    plan = flow_info.plan

    # Get steps in linear order (skip GHA actions and condition-check nodes for local execution)
    # Condition-check nodes are for GHA workflows; locally we evaluate conditions inline
    steps = [
        (n.step_name or n.name, n)
        for n in plan.nodes
        if not n.task_info.is_gha_action and not n.task_info.is_condition_check
    ]

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

        # Build command using the standalone _run_step module
        # This works regardless of whether the original script has CLI handling
        cmd = [
            sys.executable,
            "-m",
            "recompose._run_step",
        ]
        # Use --module for module entry points, --script for file paths
        if entry_type == "module":
            cmd.extend(["--module", entry_value])
        else:
            cmd.extend(["--script", entry_value])
        cmd.extend([
            "--flow",
            flow_name,
            "--step",
            step_name,
            "--workspace",
            str(ws),
        ])

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
