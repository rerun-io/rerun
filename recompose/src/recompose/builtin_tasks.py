"""
Built-in utility tasks that ship with recompose.

These tasks are always available and can be used in flows/automations
just like any user-defined task.
"""

import subprocess
from pathlib import Path
from typing import Any

from .context import out
from .gha import WorkflowSpec
from .result import Err, Ok, Result
from .task import task


def _find_git_root() -> Path | None:
    """Find the root of the git repository."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(result.stdout.strip())
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None


def _get_default_workflows_dir() -> Path | None:
    """Get the default .github/workflows directory."""
    git_root = _find_git_root()
    if git_root:
        return git_root / ".github" / "workflows"
    return None


def _workflow_filename(name: str, target_type: str) -> str:
    """Generate workflow filename with recompose prefix."""
    if target_type == "flow":
        return f"recompose_flow_{name}.yml"
    else:
        return f"recompose_automation_{name}.yml"


@task
def generate_gha(
    *,
    target: str | None = None,
    output_dir: str | None = None,
    script: str | None = None,
    runs_on: str = "ubuntu-latest",
    check_only: bool = False,
) -> Result[list[WorkflowSpec]]:
    """
    Generate GitHub Actions workflow YAML for flows and automations.

    By default, generates workflows for ALL registered flows and automations
    to .github/workflows/ in the git repository root.

    Workflow files are named:
    - recompose_flow_<name>.yml for flows
    - recompose_automation_<name>.yml for automations

    Args:
        target: Specific flow/automation to generate. If not provided, generates all.
        output_dir: Output directory for workflow files. Default: .github/workflows/
        script: Script path for workflow steps (default: auto-detect from sys.argv[0]).
        runs_on: GitHub runner to use (default: ubuntu-latest).
        check_only: If True, only check if files are up-to-date (don't write).
                   Returns Err if any files would change.

    Returns:
        List of WorkflowSpec objects that were generated.

    Examples:
        # Generate all workflows
        ./run generate_gha

        # Generate specific workflow
        ./run generate_gha --target=ci

        # Check if workflows are up-to-date (for CI)
        ./run generate_gha --check_only

        # Generate to custom directory
        ./run generate_gha --output_dir=/tmp/workflows
    """
    import sys

    from .automation import get_automation, get_automation_registry
    from .flow import get_flow, get_flow_registry
    from .gha import render_automation_workflow, render_flow_workflow

    # Determine output directory
    if output_dir:
        workflows_dir = Path(output_dir)
    else:
        workflows_dir = _get_default_workflows_dir()
        if workflows_dir is None:
            return Err("Could not find git root. Specify --output_dir explicitly.")

    # Determine script path (relative to git root)
    git_root = _find_git_root()
    if script:
        script_path = script
    elif git_root:
        # Try to make script path relative to git root
        script_abs = Path(sys.argv[0]).resolve()
        try:
            script_path = str(script_abs.relative_to(git_root))
        except ValueError:
            script_path = sys.argv[0]
    else:
        script_path = sys.argv[0]

    # Collect targets to generate
    # (short_name, target_type, info, description)
    targets: list[tuple[str, str, Any, str | None]] = []

    def _get_description(info: Any) -> str | None:
        """Extract first line of docstring as description."""
        if info.doc:
            return info.doc.strip().split("\n")[0]
        return None

    if target:
        # Specific target
        flow_info = get_flow(target)
        automation_info = get_automation(target)

        if flow_info is None and automation_info is None:
            flow_names = list(get_flow_registry().keys())
            auto_names = list(get_automation_registry().keys())
            msg = f"'{target}' not found.\n"
            if flow_names:
                msg += f"Flows: {', '.join(flow_names)}\n"
            if auto_names:
                msg += f"Automations: {', '.join(auto_names)}"
            return Err(msg)

        if flow_info:
            short_name = flow_info.name.split(":")[-1]
            targets.append((short_name, "flow", flow_info, _get_description(flow_info)))
        else:
            short_name = automation_info.name.split(":")[-1]
            targets.append((short_name, "automation", automation_info, _get_description(automation_info)))
    else:
        # All flows and automations
        for full_key, info in get_flow_registry().items():
            short_name = info.name.split(":")[-1]
            targets.append((short_name, "flow", info, _get_description(info)))
        for full_key, info in get_automation_registry().items():
            short_name = info.name.split(":")[-1]
            targets.append((short_name, "automation", info, _get_description(info)))

    if not targets:
        return Err("No flows or automations registered.")

    # Generate workflows
    results: list[WorkflowSpec] = []
    changes: list[str] = []
    errors: list[str] = []

    mode = "Checking" if check_only else "Generating"
    out(f"{mode} {len(targets)} workflow(s) to {workflows_dir}")

    for short_name, target_type, info, description in targets:
        filename = _workflow_filename(short_name, target_type)
        output_file = workflows_dir / filename

        try:
            if target_type == "flow":
                spec = render_flow_workflow(info, script_path=script_path, runs_on=runs_on)
            else:
                spec = render_automation_workflow(info)

            yaml_content = spec.to_yaml(include_header=True, source=f"{target_type}: {short_name}")

            # Determine status
            if output_file.exists():
                existing = output_file.read_text()
                if existing != yaml_content:
                    status = "updated" if not check_only else "would change"
                    changes.append(filename)
                else:
                    status = "unchanged"
            else:
                status = "created" if not check_only else "would create"
                changes.append(filename)

            # Write file if not check_only and there are changes
            if not check_only and status in ("created", "updated"):
                workflows_dir.mkdir(parents=True, exist_ok=True)
                output_file.write_text(yaml_content)

            results.append(spec)

            # Print status
            status_icon = {"created": "+", "updated": "~", "unchanged": "=",
                          "would change": "~", "would create": "+"}
            icon = status_icon.get(status, "?")
            desc = f" - {description}" if description else ""
            out(f"  [{icon}] {filename}{desc}")

        except Exception as e:
            errors.append(f"{short_name}: {e}")
            out(f"  [!] {filename} - ERROR: {e}")

    if errors:
        return Err(f"Errors generating workflows:\n" + "\n".join(errors))

    if check_only and changes:
        return Err(
            f"Workflows out of sync ({len(changes)} file(s) would change).\n"
            "Run without --check_only to update."
        )

    if check_only:
        out("All workflows up-to-date!")
    else:
        out(f"Generated {len(results)} workflow(s)")

    return Ok(results)


@task
def inspect(*, target: str, params: str | None = None) -> Result[dict[str, Any]]:
    """
    Inspect a task, flow, or automation without executing it.

    Shows signature, documentation, and for flows/automations, the task graph.

    Args:
        target: Name of the task, flow, or automation to inspect.
        params: Optional parameters for flow inspection as "key=value,key2=value2".

    Returns:
        Dict with inspection information.

    Examples:
        ./run inspect --target=lint
        ./run inspect --target=ci
        ./run inspect --target=ci --params="verbose=true"
    """
    import inspect as py_inspect

    from .automation import get_automation
    from .flow import get_flow
    from .task import get_task

    # Parse params
    kwargs: dict[str, Any] = {}
    if params:
        for pair in params.split(","):
            if "=" in pair:
                key, value = pair.split("=", 1)
                # Try to parse as bool/int/float
                if value.lower() == "true":
                    kwargs[key.strip()] = True
                elif value.lower() == "false":
                    kwargs[key.strip()] = False
                else:
                    try:
                        kwargs[key.strip()] = int(value)
                    except ValueError:
                        try:
                            kwargs[key.strip()] = float(value)
                        except ValueError:
                            kwargs[key.strip()] = value

    result: dict[str, Any] = {"target": target}

    # Try task first
    task_info = get_task(target)
    if task_info is not None:
        result["type"] = "task"
        result["module"] = task_info.module
        result["doc"] = task_info.doc

        # Build signature info
        sig_params = []
        for param_name, param in task_info.signature.parameters.items():
            if param_name == "self":
                continue
            annotation = param.annotation
            type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
            if param.default is not py_inspect.Parameter.empty:
                sig_params.append({"name": param_name, "type": type_str, "default": repr(param.default)})
            else:
                sig_params.append({"name": param_name, "type": type_str, "required": True})
        result["parameters"] = sig_params

        _print_task_info(result)
        return Ok(result)

    # Try flow
    flow_info = get_flow(target)
    if flow_info is not None:
        result["type"] = "flow"
        result["module"] = flow_info.module
        result["doc"] = flow_info.doc

        # Build signature info
        sig_params = []
        for param_name, param in flow_info.signature.parameters.items():
            annotation = param.annotation
            type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
            if param.default is not py_inspect.Parameter.empty:
                sig_params.append({"name": param_name, "type": type_str, "default": repr(param.default)})
            else:
                sig_params.append({"name": param_name, "type": type_str, "required": True})
        result["parameters"] = sig_params

        # Try to get task graph
        try:
            plan = flow_info.fn.plan(**kwargs)  # type: ignore[attr-defined]
            result["task_count"] = len(plan.nodes)

            execution_order = []
            for node in plan.get_execution_order():
                deps = [d.name for d in node.dependencies]
                execution_order.append({"name": node.name, "dependencies": deps})
            result["execution_order"] = execution_order

            if plan.terminal:
                result["terminal"] = plan.terminal.name
        except Exception as e:
            result["plan_error"] = str(e)

        _print_flow_info(result)
        return Ok(result)

    # Try automation
    automation_info = get_automation(target)
    if automation_info is not None:
        result["type"] = "automation"
        result["module"] = automation_info.module
        result["doc"] = automation_info.doc

        # Get plan
        try:
            plan = automation_info.fn.plan()  # type: ignore[attr-defined]
            result["dispatches"] = [
                {"flow": d.flow_name, "params": d.params} for d in plan.dispatches
            ]
        except Exception as e:
            result["plan_error"] = str(e)

        _print_automation_info(result)
        return Ok(result)

    # Not found
    from .automation import get_automation_registry
    from .flow import get_flow_registry
    from .task import get_registry

    task_names = list(get_registry().keys())
    flow_names = list(get_flow_registry().keys())
    auto_names = list(get_automation_registry().keys())

    msg = f"'{target}' not found.\n"
    if task_names:
        msg += f"Tasks: {', '.join(task_names)}\n"
    if flow_names:
        msg += f"Flows: {', '.join(flow_names)}\n"
    if auto_names:
        msg += f"Automations: {', '.join(auto_names)}"
    return Err(msg)


def _print_task_info(info: dict[str, Any]) -> None:
    """Print task inspection info."""
    out(f"\nTask: {info['target']}")
    out(f"Module: {info['module']}")

    if info.get("doc"):
        out(f"\nDescription: {info['doc'].strip().split(chr(10))[0]}")

    out("\nParameters:")
    for p in info.get("parameters", []):
        if p.get("required"):
            out(f"  --{p['name']}: {p['type']} [required]")
        else:
            out(f"  --{p['name']}: {p['type']} = {p['default']}")


def _print_flow_info(info: dict[str, Any]) -> None:
    """Print flow inspection info."""
    out(f"\nFlow: {info['target']}")
    out(f"Module: {info['module']}")

    if info.get("doc"):
        out(f"\nDescription: {info['doc'].strip().split(chr(10))[0]}")

    out("\nParameters:")
    params = info.get("parameters", [])
    if params:
        for p in params:
            if p.get("required"):
                out(f"  --{p['name']}: {p['type']} [required]")
            else:
                out(f"  --{p['name']}: {p['type']} = {p['default']}")
    else:
        out("  (none)")

    if info.get("plan_error"):
        out(f"\nCould not build plan: {info['plan_error']}")
    else:
        out(f"\nTask Graph ({info.get('task_count', 0)} tasks):")
        out("  Execution order:")
        for i, step in enumerate(info.get("execution_order", []), 1):
            deps = step.get("dependencies", [])
            if deps:
                out(f"    {i}. {step['name']} <- {deps}")
            else:
                out(f"    {i}. {step['name']}")

        if info.get("terminal"):
            out(f"\n  Terminal: {info['terminal']}")


def _print_automation_info(info: dict[str, Any]) -> None:
    """Print automation inspection info."""
    out(f"\nAutomation: {info['target']}")
    out(f"Module: {info['module']}")

    if info.get("doc"):
        out(f"\nDescription: {info['doc'].strip().split(chr(10))[0]}")

    if info.get("plan_error"):
        out(f"\nCould not build plan: {info['plan_error']}")
    else:
        out("\nDispatches:")
        for d in info.get("dispatches", []):
            if d.get("params"):
                out(f"  {d['flow']}({d['params']})")
            else:
                out(f"  {d['flow']}")
