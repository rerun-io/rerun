#!/usr/bin/env python3
"""
Standalone step executor for subprocess isolation.

This module provides a CLI entry point that can execute a single step of a flow
without requiring the original script to have CLI handling code.

Usage:
    # With a script path:
    python -m recompose._run_step --script /path/to/script.py --flow flow_name --step step_name --workspace /path/to/workspace

    # With a module name:
    python -m recompose._run_step --module examples.app --flow flow_name --step step_name --workspace /path/to/workspace

The script/module is imported to define the flows/tasks, then the specified step is executed.
This allows subprocess isolation to work with any Python file that defines flows,
without requiring that file to set up a recompose CLI.
"""

from __future__ import annotations

import argparse
import importlib
import importlib.util
import sys
from pathlib import Path


def main() -> None:
    """Execute a single step from a flow defined in a script or module."""
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

    # Import the script/module to define flows/tasks
    module = None

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

    # Find the flow - look through the module's attributes for FlowWrapper instances
    from .flow import FlowInfo, FlowWrapper

    flow_info: FlowInfo | None = None

    for attr_name in dir(module):
        attr = getattr(module, attr_name)
        if hasattr(attr, "_flow_info") and isinstance(attr._flow_info, FlowInfo):
            if attr._flow_info.name == flow_name:
                flow_info = attr._flow_info
                break

    if flow_info is None:
        print(f"Error: Flow '{flow_name}' not found in {script_path}", file=sys.stderr)
        print(f"Available flows: {[getattr(module, n)._flow_info.name for n in dir(module) if hasattr(getattr(module, n), '_flow_info')]}", file=sys.stderr)
        sys.exit(1)

    # Execute the step
    from .local_executor import run_step

    result = run_step(flow_info, step_name, workspace)

    if result.failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
