"""
Built-in utility tasks that ship with recompose.

These tasks are always available and can be used in automations
just like any user-defined task.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any

from .context import dbg, get_cli_command, get_working_directory, out
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


def _workflow_filename(name: str) -> str:
    """Generate workflow filename with recompose prefix."""
    return f"recompose_{name}.yml"


@task
def generate_gha(
    *,
    target: str | None = None,
    output_dir: str | None = None,
    check_only: bool = False,
) -> Result[list[Path]]:
    """
    Generate GitHub Actions workflow YAML for automations.

    By default, generates workflows for ALL registered automations
    to .github/workflows/ in the git repository root.

    Workflow files are named recompose_<name>.yml.

    Note: Dispatchables (from make_dispatchable) are automations with
    workflow_dispatch trigger, so they are included automatically.

    Args:
        target: Specific automation to generate. If not provided, generates all.
        output_dir: Output directory for workflow files. Default: .github/workflows/
        check_only: If True, only check if files are up-to-date (don't write).
                   Returns Err if any files would change.

    Returns:
        List of Path objects for files that were updated/created.
        Empty list means no files were changed.

    Examples:
        # Generate all workflows
        ./run generate-gha

        # Generate specific workflow
        ./run generate-gha --target=ci

        # Check if workflows are up-to-date (for CI)
        ./run generate-gha --check-only

        # Generate to custom directory
        ./run generate-gha --output-dir=/tmp/workflows

    """
    from .context import get_automation, get_automation_registry
    from .gha import render_automation_jobs

    # Determine output directory
    if output_dir:
        workflows_dir = Path(output_dir)
    else:
        maybe_workflows_dir = _get_default_workflows_dir()
        if maybe_workflows_dir is None:
            return Err("Could not find git root. Specify --output-dir explicitly.")
        workflows_dir = maybe_workflows_dir

    # Get configuration
    working_directory = get_working_directory()
    entry_point = get_cli_command()

    # Collect targets to generate
    # (name, obj, description)
    targets: list[tuple[str, Any, str | None]] = []

    def _get_description(info: Any) -> str | None:
        """Extract first line of docstring as description."""
        doc = getattr(info, "doc", None)
        if doc:
            first_line: str = doc.strip().split("\n")[0]
            return first_line
        return None

    if target:
        # Specific target
        automation_info = get_automation(target)

        if automation_info is None:
            auto_names = list(get_automation_registry().keys())
            msg = f"'{target}' not found.\n"
            if auto_names:
                msg += f"Automations: {', '.join(auto_names)}"
            return Err(msg)

        # Need to find the wrapper from the registry
        for full_key, auto_info in get_automation_registry().items():
            if auto_info.name == target or full_key == target:
                targets.append((auto_info.name, auto_info, _get_description(auto_info)))
                break
    else:
        # All automations
        for full_key, auto_info in get_automation_registry().items():
            targets.append((auto_info.name, auto_info, _get_description(auto_info)))

    if not targets:
        out("No automations registered.")
        return Ok([])

    # Generate workflows
    changed_paths: list[Path] = []
    errors: list[str] = []

    mode = "Checking" if check_only else "Generating"
    out(f"{mode} {len(targets)} workflow(s) to {workflows_dir}")

    for name, obj, description in targets:
        filename = _workflow_filename(name)
        output_file = workflows_dir / filename

        try:
            # obj is AutomationInfo - need to get the wrapper
            wrapper = obj.wrapper
            spec = render_automation_jobs(
                wrapper,
                entry_point=entry_point,
                working_directory=working_directory,
            )

            # Set the output path on the spec
            spec.path = output_file

            yaml_content = spec.to_yaml(include_header=True, source=f"automation: {name}")

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
                errors.append(f"{name}: actionlint: {validation_msg}")

            # Print status
            status_icon = {"created": "+", "updated": "~", "unchanged": "=", "would change": "~", "would create": "+"}
            icon = status_icon.get(status, "?")
            desc = f" - {description}" if description else ""
            out(f"  [{icon}] {filename}{desc}")

        except Exception as e:
            errors.append(f"{name}: {e}")
            out(f"  [!] {filename} - ERROR: {e}")

    if errors:
        return Err("Errors generating workflows:\n" + "\n".join(errors))

    if check_only and changed_paths:
        return Err(
            f"Workflows out of sync ({len(changed_paths)} file(s) would change).\nRun without --check-only to update."
        )

    if check_only:
        out("All workflows up-to-date!")
    else:
        out(f"Generated {len(targets)} workflow(s)")

    return Ok(changed_paths)


@task
def inspect(*, target: str) -> Result[None]:
    """
    Inspect a task or automation without executing it.

    Shows signature, documentation, and for automations, the job list.

    Args:
        target: Name of the task or automation to inspect.

    Examples:
        ./run inspect --target=lint
        ./run inspect --target=ci

    """
    import inspect as py_inspect

    from .context import get_automation, get_task

    # Try task first
    task_info = get_task(target)
    if task_info is not None:
        _print_task_info(task_info, py_inspect)
        return Ok(None)

    # Try automation (includes dispatchables, which are automations with workflow_dispatch trigger)
    automation_info = get_automation(target)
    if automation_info is not None:
        _print_automation_info(automation_info, py_inspect)
        return Ok(None)

    # Not found
    from .context import get_automation_registry, get_task_registry

    task_names = list(get_task_registry().keys())
    auto_names = list(get_automation_registry().keys())

    msg = f"'{target}' not found.\n"
    if task_names:
        msg += f"Tasks: {', '.join(task_names)}\n"
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

    # Show task decorator parameters if present
    if task_info.outputs:
        out(f"\nOutputs: {', '.join(task_info.outputs)}")
    if task_info.artifacts:
        out(f"Artifacts: {', '.join(task_info.artifacts)}")
    if task_info.secrets:
        out(f"Secrets: {', '.join(task_info.secrets)}")


def _print_automation_info(automation_info: Any, py_inspect: Any) -> None:
    """Print automation inspection info."""
    out(f"\nAutomation: {automation_info.name}")
    out(f"Module: {automation_info.module}")

    if automation_info.doc:
        out(f"\nDescription: {automation_info.doc.strip().split(chr(10))[0]}")

    # Show trigger if present
    if automation_info.trigger:
        out(f"\nTrigger: {automation_info.trigger}")

    # Show input parameters
    if automation_info.input_params:
        out("\nInputs:")
        for name, param in automation_info.input_params.items():
            default_str = f" = {param._default!r}" if param._default is not None else ""
            required_str = " [required]" if param._required else ""
            out(f"  --{name}{default_str}{required_str}")

    # Get jobs from the plan
    try:
        wrapper = automation_info.wrapper
        jobs = wrapper.plan()
        out(f"\nJobs ({len(jobs)}):")
        for job in jobs:
            needs_str = ""
            deps = job.get_all_dependencies()
            if deps:
                needs_str = f" (needs: {', '.join(d.job_id for d in deps)})"
            condition_str = ""
            if job.condition:
                condition_str = f" [if: {job.condition.to_gha_expr()}]"
            out(f"  - {job.job_id}{needs_str}{condition_str}")
    except Exception as e:
        out(f"\nCould not build job plan: {e}")


def builtin_commands() -> CommandGroup:
    """
    Returns a CommandGroup containing all built-in recompose commands.

    Built-in commands:
        - generate_gha: Generate GitHub Actions workflow YAML
        - inspect: Inspect tasks or automations

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
