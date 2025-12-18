#!/usr/bin/env python3
"""
Standalone step executor for subprocess isolation.

This module provides a CLI entry point that can execute a single step of a flow
without requiring the original script to have CLI handling code.

Usage:
    # With a script path:
    python -m recompose._run_step --script /path/to/script.py \\
        --flow flow_name --step step_name --workspace /path/to/workspace

    # With a module name:
    python -m recompose._run_step --module examples.app \\
        --flow flow_name --step step_name --workspace /path/to/workspace

The script/module is imported to define the flows/tasks, then the specified step
is executed. This allows subprocess isolation to work with any Python file that
defines flows, without requiring that file to set up a recompose CLI.

When the module contains a recompose.App instance (recommended pattern), the app's
configuration (working_directory, python_cmd, etc.) is automatically applied.
"""

from __future__ import annotations

import argparse
import importlib
import importlib.util
import sys
from pathlib import Path
from types import ModuleType
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .command_group import App
    from .flow import FlowInfo


def _find_app(module: ModuleType) -> App | None:
    """Find a recompose.App instance in the module."""
    from .command_group import App

    for attr_name in dir(module):
        attr = getattr(module, attr_name)
        if isinstance(attr, App):
            return attr
    return None


def _find_flow_info(module: ModuleType, flow_name: str) -> FlowInfo | None:
    """Find a FlowInfo by name from module attributes."""
    from .flow import FlowInfo

    for attr_name in dir(module):
        attr = getattr(module, attr_name)
        if hasattr(attr, "_flow_info") and isinstance(attr._flow_info, FlowInfo):
            if attr._flow_info.name == flow_name:
                return attr._flow_info
    return None


def _get_available_flows(module: ModuleType) -> list[str]:
    """Get list of available flow names from module attributes."""
    from .flow import FlowInfo

    flows = []
    for attr_name in dir(module):
        attr = getattr(module, attr_name)
        if hasattr(attr, "_flow_info") and isinstance(attr._flow_info, FlowInfo):
            flows.append(attr._flow_info.name)
    return flows


def main() -> None:
    """Execute a single step from a flow defined in a script or module."""
    from .context import set_entry_point
    from .flow import FlowInfo

    parser = argparse.ArgumentParser(description="Execute a single flow step")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--script", type=Path, help="Path to the script defining the flow")
    group.add_argument("--module", type=str, help="Module name defining the flow (e.g., examples.app)")
    parser.add_argument("--flow", type=str, required=True, help="Flow name")
    parser.add_argument("--step", type=str, required=True, help="Step name to execute")
    parser.add_argument("--workspace", type=Path, required=True, help="Workspace directory")

    args = parser.parse_args()

    flow_name: str = args.flow
    step_name: str = args.step
    workspace: Path = args.workspace

    # Set the entry point so tasks like generate_gha can determine the correct script path
    if args.module:
        set_entry_point("module", args.module)
    else:
        set_entry_point("script", str(args.script))

    # Import the script/module to define flows/tasks
    module: ModuleType | None = None

    if args.module:
        # Import by module name
        try:
            module = importlib.import_module(args.module)
        except ImportError as e:
            print(f"Error: Could not import module '{args.module}': {e}", file=sys.stderr)
            sys.exit(1)
    else:
        # Import by script path
        script_path: Path = args.script
        if not script_path.exists():
            print(f"Error: Script not found: {script_path}", file=sys.stderr)
            sys.exit(1)

        spec = importlib.util.spec_from_file_location("__recompose_script__", script_path)
        if spec is None or spec.loader is None:
            print(f"Error: Could not load script: {script_path}", file=sys.stderr)
            sys.exit(1)

        module = importlib.util.module_from_spec(spec)
        sys.modules["__recompose_script__"] = module

        try:
            spec.loader.exec_module(module)
        except Exception as e:
            print(f"Error loading script {script_path}: {e}", file=sys.stderr)
            sys.exit(1)

    assert module is not None

    # Look for a recompose.App instance in the module
    # This is the recommended pattern - it provides config and registrations
    app = _find_app(module)

    if app is not None:
        # Use the app to set up context (working_directory, python_cmd, registries)
        app.setup_context()

        # Find the flow from the app's context
        from .context import get_flow, get_flow_registry

        flow_info = get_flow(flow_name)
        if flow_info is None:
            available = list(get_flow_registry().keys())
            print(f"Error: Flow '{flow_name}' not found in app", file=sys.stderr)
            print(f"Available flows: {available}", file=sys.stderr)
            sys.exit(1)
    else:
        # Fallback: scan module for FlowWrapper/TaskWrapper directly
        # This works for simple cases but won't have config like working_directory
        flow_info = _find_flow_info(module, flow_name)

        if flow_info is None:
            available = _get_available_flows(module)
            print(f"Error: Flow '{flow_name}' not found in module", file=sys.stderr)
            print(f"Available flows: {available}", file=sys.stderr)
            sys.exit(1)

        # Set up minimal context from module scanning
        from .context import RecomposeContext, set_recompose_context
        from .task import TaskInfo

        all_flows: dict[str, FlowInfo] = {}
        all_tasks: dict[str, TaskInfo] = {}

        for attr_name in dir(module):
            attr = getattr(module, attr_name)
            if hasattr(attr, "_flow_info") and isinstance(attr._flow_info, FlowInfo):
                fi = attr._flow_info
                all_flows[fi.name] = fi
            if hasattr(attr, "_task_info") and isinstance(attr._task_info, TaskInfo):
                ti = attr._task_info
                all_tasks[ti.name] = ti

        ctx = RecomposeContext(
            tasks=all_tasks,
            flows=all_flows,
            automations={},
        )
        set_recompose_context(ctx)

    # Execute the step
    from .local_executor import run_step

    result = run_step(flow_info, step_name, workspace)

    if result.failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
