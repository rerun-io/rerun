"""
Built-in utility tasks that ship with recompose.

These tasks are always available and can be used in flows/automations
just like any user-defined task.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any

from .context import dbg, get_entry_point, get_python_cmd, get_working_directory, out
from .gh_cli import find_git_root
from .gha import validate_workflow
from .result import Err, Ok, Result
from .task import task

if TYPE_CHECKING:
    from .command_group import CommandGroup


def _get_default_workflows_dir() -> Path | None:
    """Get the default .github/workflows directory."""
    git_root = find_git_root()
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
) -> Result[list[Path]]:
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
        List of Path objects for files that were updated/created.
        Empty list means no files were changed.

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

    from .context import get_automation, get_automation_registry, get_flow, get_flow_registry
    from .gha import render_automation_workflow, render_flow_workflow

    # Determine output directory
    if output_dir:
        workflows_dir = Path(output_dir)
    else:
        maybe_workflows_dir = _get_default_workflows_dir()
        if maybe_workflows_dir is None:
            return Err("Could not find git root. Specify --output_dir explicitly.")
        workflows_dir = maybe_workflows_dir

    # Determine script path (relative to git root or working_directory)
    # Use entry_point info to construct the correct invocation
    git_root = find_git_root()
    working_dir = get_working_directory()
    entry_point = get_entry_point()

    if script:
        script_path = script
    elif entry_point and entry_point[0] == "module":
        # Running as a module - use -m style invocation
        module_name = entry_point[1]
        script_path = f"-m {module_name}"
    elif git_root:
        # Try to make script path relative to git root
        script_abs = Path(sys.argv[0]).resolve()
        try:
            script_path = str(script_abs.relative_to(git_root))
        except ValueError:
            script_path = sys.argv[0]

        # If working_directory is set, adjust script_path to be relative to it
        if working_dir and script_path.startswith(working_dir + "/"):
            script_path = script_path[len(working_dir) + 1 :]
    else:
        script_path = sys.argv[0]

    # Collect targets to generate
    # (short_name, target_type, info, description)
    targets: list[tuple[str, str, Any, str | None]] = []

    def _get_description(info: Any) -> str | None:
        """Extract first line of docstring as description."""
        if info.doc:
            first_line: str = info.doc.strip().split("\n")[0]
            return first_line
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
            assert automation_info is not None  # We checked both aren't None above
            short_name = automation_info.name.split(":")[-1]
            targets.append((short_name, "automation", automation_info, _get_description(automation_info)))
    else:
        # All flows and automations
        for full_key, flow in get_flow_registry().items():
            short_name = flow.name.split(":")[-1]
            targets.append((short_name, "flow", flow, _get_description(flow)))
        for full_key, auto in get_automation_registry().items():
            short_name = auto.name.split(":")[-1]
            targets.append((short_name, "automation", auto, _get_description(auto)))

    if not targets:
        return Err("No flows or automations registered.")

    # Generate workflows
    changed_paths: list[Path] = []
    errors: list[str] = []

    mode = "Checking" if check_only else "Generating"
    out(f"{mode} {len(targets)} workflow(s) to {workflows_dir}")

    for short_name, target_type, info, description in targets:
        filename = _workflow_filename(short_name, target_type)
        output_file = workflows_dir / filename

        try:
            if target_type == "flow":
                spec = render_flow_workflow(
                    info,
                    script_path=script_path,
                    runs_on=runs_on,
                    python_cmd=get_python_cmd(),
                    working_directory=get_working_directory(),
                )
            else:
                spec = render_automation_workflow(info)

            # Set the output path on the spec
            spec.path = output_file

            yaml_content = spec.to_yaml(include_header=True, source=f"{target_type}: {short_name}")

            # Determine status
            if output_file.exists():
                existing = output_file.read_text()
                if existing != yaml_content:
                    status = "updated" if not check_only else "would change"
                    changed_paths.append(output_file)
                else:
                    status = "unchanged"
            else:
                status = "created" if not check_only else "would create"
                changed_paths.append(output_file)

            # Write file if not check_only and there are changes
            if not check_only and status in ("created", "updated"):
                workflows_dir.mkdir(parents=True, exist_ok=True)
                output_file.write_text(yaml_content)

            # Validate with actionlint if available
            valid, validation_msg = validate_workflow(yaml_content, output_file)
            if valid:
                dbg(f"actionlint: {filename} passed validation")
            elif "not found" in validation_msg:
                dbg("actionlint: not available, skipping validation")
            else:
                dbg(f"actionlint: {filename} FAILED validation")
                errors.append(f"{short_name}: actionlint: {validation_msg}")

            # Print status
            status_icon = {"created": "+", "updated": "~", "unchanged": "=", "would change": "~", "would create": "+"}
            icon = status_icon.get(status, "?")
            desc = f" - {description}" if description else ""
            out(f"  [{icon}] {filename}{desc}")

        except Exception as e:
            errors.append(f"{short_name}: {e}")
            out(f"  [!] {filename} - ERROR: {e}")

    if errors:
        return Err("Errors generating workflows:\n" + "\n".join(errors))

    if check_only and changed_paths:
        return Err(
            f"Workflows out of sync ({len(changed_paths)} file(s) would change).\nRun without --check_only to update."
        )

    if check_only:
        out("All workflows up-to-date!")
    else:
        out(f"Generated {len(targets)} workflow(s)")

    return Ok(changed_paths)


@task
def inspect(*, target: str) -> Result[None]:
    """
    Inspect a task, flow, or automation without executing it.

    Shows signature, documentation, and for flows/automations, the task graph.

    Args:
        target: Name of the task, flow, or automation to inspect.

    Examples:
        ./run inspect --target=lint
        ./run inspect --target=ci

    """
    import inspect as py_inspect

    from .context import get_automation, get_flow, get_task

    # Try task first
    task_info = get_task(target)
    if task_info is not None:
        _print_task_info(task_info, py_inspect)
        return Ok(None)

    # Try flow
    flow_info = get_flow(target)
    if flow_info is not None:
        _print_flow_info(flow_info, py_inspect)
        return Ok(None)

    # Try automation
    automation_info = get_automation(target)
    if automation_info is not None:
        _print_automation_info(automation_info, py_inspect)
        return Ok(None)

    # Not found
    from .context import get_automation_registry, get_flow_registry, get_task_registry

    task_names = list(get_task_registry().keys())
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


def _print_task_info(task_info: Any, py_inspect: Any) -> None:
    """Print task inspection info."""
    out(f"\nTask: {task_info.name}")
    out(f"Module: {task_info.module}")

    if task_info.doc:
        out(f"\nDescription: {task_info.doc.strip().split(chr(10))[0]}")

    out("\nParameters:")
    has_params = False
    for param_name, param in task_info.signature.parameters.items():
        if param_name == "self":
            continue
        has_params = True
        annotation = param.annotation
        type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
        if param.default is not py_inspect.Parameter.empty:
            out(f"  --{param_name}: {type_str} = {param.default!r}")
        else:
            out(f"  --{param_name}: {type_str} [required]")
    if not has_params:
        out("  (none)")


def _print_flow_info(flow_info: Any, py_inspect: Any) -> None:
    """Print flow inspection info."""
    from .plan import FlowPlan

    out(f"\nFlow: {flow_info.name}")
    out(f"Module: {flow_info.module}")

    if flow_info.doc:
        out(f"\nDescription: {flow_info.doc.strip().split(chr(10))[0]}")

    out("\nParameters:")
    has_params = False
    for param_name, param in flow_info.signature.parameters.items():
        has_params = True
        annotation = param.annotation
        type_str = annotation.__name__ if hasattr(annotation, "__name__") else str(annotation)
        if param.default is not py_inspect.Parameter.empty:
            out(f"  --{param_name}: {type_str} = {param.default!r}")
        else:
            out(f"  --{param_name}: {type_str} [required]")
    if not has_params:
        out("  (none)")

    # Get the plan
    plan: FlowPlan = flow_info.plan
    out(f"\nTask Graph ({len(plan.nodes)} steps):")
    _print_task_tree(plan)


def _print_task_tree(plan: Any) -> None:
    """Print a tree visualization of the flow's task graph."""
    if not plan.nodes:
        out("  (empty)")
        return

    # Group conditional tasks by their condition_check_step
    conditional_tasks: dict[str, list[Any]] = {}
    for node in plan.nodes:
        if node.condition_check_step:
            conditional_tasks.setdefault(node.condition_check_step, []).append(node)

    # Track which nodes we've printed (to skip conditional tasks printed under run_if)
    printed: set[str] = set()

    # Get top-level nodes (not gated by a condition)
    top_level = [n for n in plan.nodes if not n.condition_check_step]

    for i, node in enumerate(top_level):
        is_last = i == len(top_level) - 1
        connector = "└─" if is_last else "├─"
        cont_prefix = "   " if is_last else "│  "

        # Check if this is a condition-check node (run_if_N)
        is_condition_check = getattr(node.task_info, "is_condition_check", False)

        if is_condition_check and node.step_name:
            # Print the run_if node with its condition
            condition_data = node.kwargs.get("condition_data", {})
            condition_str = _format_condition(condition_data)
            out(f"  {connector} {node.step_name} [if: {condition_str}]")
            printed.add(node.name)

            # Print nested conditional tasks
            nested = conditional_tasks.get(node.step_name, [])
            for j, nested_node in enumerate(nested):
                nested_is_last = j == len(nested) - 1
                nested_connector = "└─" if nested_is_last else "├─"
                out(f"  {cont_prefix}{nested_connector} {nested_node.name}")
                printed.add(nested_node.name)

                # Show dependencies for nested task
                if nested_node.dependencies:
                    dep_names = [d.name for d in nested_node.dependencies]
                    nested_cont = "   " if nested_is_last else "│  "
                    out(f"  {cont_prefix}{nested_cont}  depends: {', '.join(dep_names)}")
        else:
            # Regular task
            out(f"  {connector} {node.name}")
            printed.add(node.name)

            # Show dependencies if any
            if node.dependencies:
                dep_names = [d.name for d in node.dependencies]
                out(f"  {cont_prefix}  depends: {', '.join(dep_names)}")


def _format_condition(condition_data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    if not condition_data:
        return "?"

    expr_type = condition_data.get("type")
    if expr_type == "input":
        return str(condition_data.get("name", "?"))
    elif expr_type == "literal":
        return repr(condition_data.get("value"))
    elif expr_type == "binary":
        left = _format_condition(condition_data.get("left", {}))
        op = condition_data.get("op", "?")
        right = _format_condition(condition_data.get("right", {}))
        return f"{left} {op} {right}"
    elif expr_type == "unary":
        op = condition_data.get("op", "?")
        operand = _format_condition(condition_data.get("operand", {}))
        return f"{op} {operand}"
    return "?"


def _print_automation_info(automation_info: Any, py_inspect: Any) -> None:
    """Print automation inspection info."""
    out(f"\nAutomation: {automation_info.name}")
    out(f"Module: {automation_info.module}")

    if automation_info.doc:
        out(f"\nDescription: {automation_info.doc.strip().split(chr(10))[0]}")

    # Get plan
    try:
        plan = automation_info.fn.plan()
        out("\nDispatches:")
        for d in plan.dispatches:
            if d.params:
                params_str = ", ".join(f"{k}={v!r}" for k, v in d.params.items())
                out(f"  {d.flow_name}({params_str})")
            else:
                out(f"  {d.flow_name}()")
    except Exception as e:
        out(f"\nCould not build plan: {e}")


def builtin_commands() -> CommandGroup:
    """
    Returns a CommandGroup containing all built-in recompose commands.

    Built-in commands:
        - generate_gha: Generate GitHub Actions workflow YAML
        - inspect: Inspect tasks, flows, or automations

    Example:
        commands = [
            recompose.CommandGroup("Quality", [lint, test]),
            recompose.builtin_commands(),  # Adds generate_gha, inspect
        ]
        recompose.main(commands=commands)

    Returns
    -------
        CommandGroup with built-in commands under "Built-in" heading.

    """
    from .command_group import CommandGroup

    return CommandGroup("Built-in", [generate_gha, inspect])
