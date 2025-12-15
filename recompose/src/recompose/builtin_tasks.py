"""
Built-in utility tasks that ship with recompose.

These tasks are always available and can be used in flows/automations
just like any user-defined task.
"""

from pathlib import Path
from typing import Any

from .context import out
from .result import Err, Ok, Result
from .task import task


@task
def generate_gha(
    *,
    target: str,
    output: str | None = None,
    script: str | None = None,
    runs_on: str = "ubuntu-latest",
    validate: bool = True,
    include_header: bool = True,
) -> Result[str]:
    """
    Generate GitHub Actions workflow YAML for a flow or automation.

    Args:
        target: Name of the flow or automation to generate workflow for.
        output: Output file path. If not provided, prints to stdout.
        script: Script path for workflow steps (default: auto-detect from sys.argv[0]).
        runs_on: GitHub runner to use (default: ubuntu-latest).
        validate: Validate generated YAML with actionlint (default: True).
        include_header: Include "GENERATED FILE" header comment (default: True).

    Returns:
        The generated YAML content.

    Examples:
        # Generate and print to stdout
        ./run generate_gha --target=ci

        # Generate to file
        ./run generate_gha --target=ci --output=.github/workflows/ci.yml

        # Skip validation
        ./run generate_gha --target=ci --no-validate
    """
    import sys

    from .automation import get_automation
    from .flow import get_flow
    from .gha import render_automation_workflow, render_flow_workflow, validate_workflow

    # Find target as flow or automation
    flow_info = get_flow(target)
    automation_info = get_automation(target)

    if flow_info is None and automation_info is None:
        # List available options
        from .automation import get_automation_registry
        from .flow import get_flow_registry

        flow_names = list(get_flow_registry().keys())
        auto_names = list(get_automation_registry().keys())

        msg = f"'{target}' not found as flow or automation.\n"
        if flow_names:
            msg += f"Available flows: {', '.join(flow_names)}\n"
        if auto_names:
            msg += f"Available automations: {', '.join(auto_names)}"
        return Err(msg)

    # Determine script path
    script_path = script if script else sys.argv[0]

    # Generate workflow
    try:
        if flow_info is not None:
            spec = render_flow_workflow(flow_info, script_path=script_path, runs_on=runs_on)
            source = f"flow: {flow_info.name}"
        else:
            spec = render_automation_workflow(automation_info)
            source = f"automation: {automation_info.name}"

        yaml_content = spec.to_yaml(include_header=include_header, source=source)
    except ValueError as e:
        return Err(str(e))

    # Validate if requested
    if validate:
        success, message = validate_workflow(yaml_content)
        if not success:
            if "not found" in message:
                out(f"Warning: actionlint not found, skipping validation")
            else:
                return Err(f"Validation failed:\n{message}")
        else:
            out("actionlint validation passed")

    # Output
    if output:
        output_path = Path(output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(yaml_content)
        out(f"Wrote workflow to {output_path}")

    return Ok(yaml_content)


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
