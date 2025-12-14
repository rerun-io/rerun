"""GitHub Actions workflow generation for recompose flows.

This module provides:
- Dataclasses for representing GHA workflow structure
- Functions to generate workflow YAML from flows
- Validation via actionlint
"""

from __future__ import annotations

import inspect
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from .flow import FlowInfo, get_flow


@dataclass
class StepSpec:
    """A step within a GHA job."""

    name: str
    run: str | None = None
    uses: str | None = None
    with_: dict[str, Any] | None = None
    env: dict[str, str] | None = None
    id: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        d: dict[str, Any] = {"name": self.name}
        if self.id:
            d["id"] = self.id
        if self.uses:
            d["uses"] = self.uses
        if self.with_:
            d["with"] = self.with_
        if self.run:
            d["run"] = self.run
        if self.env:
            d["env"] = self.env
        return d


@dataclass
class JobSpec:
    """A job within a GHA workflow."""

    name: str
    runs_on: str = "ubuntu-latest"
    steps: list[StepSpec] = field(default_factory=list)
    env: dict[str, str] | None = None
    timeout_minutes: int | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        d: dict[str, Any] = {"runs-on": self.runs_on}
        if self.env:
            d["env"] = self.env
        if self.timeout_minutes:
            d["timeout-minutes"] = self.timeout_minutes
        d["steps"] = [s.to_dict() for s in self.steps]
        return d


@dataclass
class WorkflowDispatchInput:
    """An input for workflow_dispatch trigger."""

    name: str
    description: str
    required: bool = False
    default: str | None = None
    type: str = "string"  # string, boolean, choice, number

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        d: dict[str, Any] = {
            "description": self.description,
            "required": self.required,
            "type": self.type,
        }
        if self.default is not None:
            d["default"] = self.default
        return d


@dataclass
class WorkflowSpec:
    """A complete GHA workflow."""

    name: str
    on: dict[str, Any]
    jobs: dict[str, JobSpec]

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        return {
            "name": self.name,
            "on": self.on,
            "jobs": {name: job.to_dict() for name, job in self.jobs.items()},
        }

    def to_yaml(self) -> str:
        """Render as YAML string."""
        # Custom representer to handle multi-line strings nicely
        def str_representer(dumper: yaml.Dumper, data: str) -> yaml.ScalarNode:
            if "\n" in data:
                return dumper.represent_scalar("tag:yaml.org,2002:str", data, style="|")
            return dumper.represent_scalar("tag:yaml.org,2002:str", data)

        yaml.add_representer(str, str_representer)

        return yaml.dump(
            self.to_dict(),
            default_flow_style=False,
            sort_keys=False,
            allow_unicode=True,
            width=120,
        )


def _python_type_to_gha_input_type(annotation: Any) -> str:
    """Map Python type annotation to GHA input type."""
    if annotation is bool:
        return "boolean"
    if annotation is int or annotation is float:
        return "number"
    # Default to string for str, Path, and anything else
    return "string"


def _default_to_string(value: Any) -> str | None:
    """Convert a Python default value to string for GHA input."""
    if value is None or value is inspect.Parameter.empty:
        return None
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, Path):
        return str(value)
    return str(value)


def _flow_params_to_inputs(flow_info: FlowInfo) -> list[WorkflowDispatchInput]:
    """Extract workflow_dispatch inputs from flow signature."""
    inputs: list[WorkflowDispatchInput] = []

    for param_name, param in flow_info.signature.parameters.items():
        annotation = param.annotation
        if annotation is inspect.Parameter.empty:
            annotation = str

        has_default = param.default is not inspect.Parameter.empty
        default_value = _default_to_string(param.default) if has_default else None

        inputs.append(
            WorkflowDispatchInput(
                name=param_name,
                description=f"Parameter: {param_name}",
                required=not has_default,
                default=default_value,
                type=_python_type_to_gha_input_type(annotation),
            )
        )

    return inputs


def _build_setup_step(flow_info: FlowInfo, script_path: str) -> StepSpec:
    """Build the setup step that initializes the workspace."""
    inputs = _flow_params_to_inputs(flow_info)

    # Build the run command with all input parameters
    cmd_parts = [
        "python",
        script_path,
        flow_info.name,
        "--setup",
        "--workspace",
        ".recompose",
    ]

    # Add each parameter from workflow_dispatch inputs
    for inp in inputs:
        cmd_parts.append(f"--{inp.name}")
        cmd_parts.append(f"${{{{ inputs.{inp.name} }}}}")

    return StepSpec(
        name="Setup workspace",
        run=" ".join(cmd_parts),
    )


def _build_task_step(step_name: str, flow_name: str, script_path: str) -> StepSpec:
    """Build a step that executes a single task."""
    return StepSpec(
        name=step_name,
        run=f"python {script_path} {flow_name} --step {step_name} --workspace .recompose",
    )


def render_flow_workflow(
    flow_info: FlowInfo,
    script_path: str = "app.py",
    runs_on: str = "ubuntu-latest",
) -> WorkflowSpec:
    """
    Generate a WorkflowSpec from a flow.

    Args:
        flow_info: The flow to generate a workflow for.
        script_path: Path to the script that contains the flow (relative to repo root).
        runs_on: The runner to use for the job.

    Returns:
        A WorkflowSpec that can be rendered to YAML.
    """
    # Build workflow_dispatch inputs from flow parameters
    inputs = _flow_params_to_inputs(flow_info)
    inputs_dict = {inp.name: inp.to_dict() for inp in inputs}

    # Build the on trigger
    on_trigger: dict[str, Any] = {"workflow_dispatch": {}}
    if inputs_dict:
        on_trigger["workflow_dispatch"]["inputs"] = inputs_dict

    # Build the plan to get step names
    # We need to call plan() with default values for required params
    # For now, we'll just use empty dict and let it fail if params are required
    try:
        # Try to build plan with no args (works if all params have defaults)
        plan = flow_info.fn.plan()  # type: ignore[attr-defined]
    except TypeError:
        # If that fails, we need to handle required params differently
        # For now, create a plan with placeholder values
        # This is a limitation - flows with required params need special handling
        raise ValueError(
            f"Flow '{flow_info.name}' has required parameters. "
            "Cannot generate workflow without default values for all parameters."
        )

    plan.assign_step_names()
    steps_info = plan.get_steps()

    # Build job steps
    job_steps: list[StepSpec] = []

    # 1. Checkout
    job_steps.append(
        StepSpec(
            name="Checkout",
            uses="actions/checkout@v4",
        )
    )

    # 2. Setup step
    job_steps.append(_build_setup_step(flow_info, script_path))

    # 3. Task steps
    for step_name, _node in steps_info:
        job_steps.append(_build_task_step(step_name, flow_info.name, script_path))

    # Build the job
    job = JobSpec(
        name=flow_info.name,
        runs_on=runs_on,
        steps=job_steps,
    )

    # Build the workflow
    return WorkflowSpec(
        name=flow_info.name,
        on=on_trigger,
        jobs={flow_info.name: job},
    )


def generate_workflow_yaml(
    flow_name: str,
    script_path: str = "app.py",
    runs_on: str = "ubuntu-latest",
) -> str:
    """
    Generate workflow YAML for a flow by name.

    Args:
        flow_name: Name of the flow to generate workflow for.
        script_path: Path to the script containing the flow.
        runs_on: The runner to use.

    Returns:
        YAML string for the workflow.

    Raises:
        ValueError: If flow not found.
    """
    flow_info = get_flow(flow_name)
    if flow_info is None:
        raise ValueError(f"Flow '{flow_name}' not found")

    spec = render_flow_workflow(flow_info, script_path=script_path, runs_on=runs_on)
    return spec.to_yaml()


def validate_workflow(yaml_content: str, filepath: Path | None = None) -> tuple[bool, str]:
    """
    Validate workflow YAML using actionlint.

    Args:
        yaml_content: The YAML content to validate.
        filepath: Optional filepath for error messages.

    Returns:
        Tuple of (success, message). If success is False, message contains errors.
    """
    # Check if actionlint is installed
    actionlint_path = shutil.which("actionlint")
    if actionlint_path is None:
        return False, (
            "actionlint not found. Install with:\n"
            "  brew install actionlint\n"
            "  # or\n"
            "  go install github.com/rhysd/actionlint/cmd/actionlint@latest"
        )

    # Write to temp file if no filepath provided
    import tempfile

    if filepath is None:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yml", delete=False) as f:
            f.write(yaml_content)
            temp_path = Path(f.name)
        try:
            result = subprocess.run(
                [actionlint_path, str(temp_path)],
                capture_output=True,
                text=True,
            )
        finally:
            temp_path.unlink()
    else:
        # Validate existing file
        result = subprocess.run(
            [actionlint_path, str(filepath)],
            capture_output=True,
            text=True,
        )

    if result.returncode == 0:
        return True, "Validation passed"
    else:
        return False, result.stdout + result.stderr
