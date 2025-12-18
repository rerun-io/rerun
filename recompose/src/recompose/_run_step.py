#!/usr/bin/env python3
"""
Standalone step executor for subprocess isolation.

This module provides a CLI entry point that can execute a single step of a flow
or set up a workspace, without requiring the original script to have CLI handling code.

Usage:
    # Setup workspace for a flow:
    python -m recompose._run_step --module examples.app --setup --flow flow_name [param=value ...]

    # Execute a single step:
    python -m recompose._run_step --module examples.app --flow flow_name --step step_name

The module is imported to find the recompose.App instance, which provides the
configuration and registered flows/tasks.
"""

from __future__ import annotations

import argparse
import importlib
import sys
from types import ModuleType
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .command_group import App


def _find_app(module: ModuleType) -> App | None:
    """Find a recompose.App instance in the module."""
    from .command_group import App

    for attr_name in dir(module):
        attr = getattr(module, attr_name)
        if isinstance(attr, App):
            return attr
    return None


def _parse_param(param: str) -> tuple[str, str]:
    """Parse a key=value parameter string."""
    if "=" not in param:
        raise ValueError(f"Invalid parameter format: {param} (expected key=value)")
    key, value = param.split("=", 1)
    return key, value


def main() -> None:
    """Execute a flow step or setup workspace."""
    parser = argparse.ArgumentParser(
        description="Execute a flow step or setup workspace",
        epilog="Parameters can be passed as key=value after the other arguments",
    )
    parser.add_argument("--module", type=str, required=True, help="Module name containing the App (e.g., examples.app)")
    parser.add_argument("--flow", type=str, required=True, help="Flow name")
    parser.add_argument("--step", type=str, help="Step name to execute (omit for --setup)")
    parser.add_argument("--setup", action="store_true", help="Setup workspace instead of running a step")
    parser.add_argument("params", nargs="*", help="Flow parameters as key=value pairs")

    args = parser.parse_args()

    module_name: str = args.module
    flow_name: str = args.flow

    # Parse parameters
    params: dict[str, str] = {}
    for param in args.params:
        try:
            key, value = _parse_param(param)
            # Convert string values to appropriate types
            if value.lower() == "true":
                params[key] = True  # type: ignore[assignment]
            elif value.lower() == "false":
                params[key] = False  # type: ignore[assignment]
            else:
                params[key] = value
        except ValueError as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)

    # Import the module
    try:
        module = importlib.import_module(module_name)
    except ImportError as e:
        print(f"Error: Could not import module '{module_name}': {e}", file=sys.stderr)
        sys.exit(1)

    # Find the recompose.App instance in the module
    app = _find_app(module)
    if app is None:
        print(f"Error: No recompose.App instance found in module '{module_name}'", file=sys.stderr)
        sys.exit(1)

    # Set up context from the app (this sets module_name, registers flows/tasks, etc.)
    app.setup_context()

    # Find the flow from the app's context
    from .context import get_flow, get_flow_registry

    flow_info = get_flow(flow_name)
    if flow_info is None:
        available = list(get_flow_registry().keys())
        print(f"Error: Flow '{flow_name}' not found in app", file=sys.stderr)
        print(f"Available flows: {available}", file=sys.stderr)
        sys.exit(1)

    if args.setup:
        # Setup workspace mode
        from .local_executor import setup_workspace
        from .workspace import get_workspace_from_env

        workspace = get_workspace_from_env()
        ws = setup_workspace(flow_info, workspace=workspace, **params)
        print(f"Workspace initialized: {ws}")
    else:
        # Execute step mode
        if not args.step:
            print("Error: --step is required when not using --setup", file=sys.stderr)
            sys.exit(1)

        from .local_executor import run_step
        from .workspace import get_workspace_from_env

        workspace = get_workspace_from_env()
        if workspace is None:
            print("Error: RECOMPOSE_WORKSPACE environment variable not set", file=sys.stderr)
            sys.exit(1)

        result = run_step(flow_info, args.step, workspace)

        if result.failed:
            sys.exit(1)


if __name__ == "__main__":
    main()
