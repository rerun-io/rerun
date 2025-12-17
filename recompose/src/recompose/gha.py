"""GitHub Actions workflow generation for recompose flows.

This module provides:
- Dataclasses for representing GHA workflow structure
- Virtual tasks for GHA-specific actions (checkout, setup-python, etc.)
- Functions to generate workflow YAML from flows
- Validation via actionlint
"""

from __future__ import annotations

import inspect
import shutil
import subprocess
from dataclasses import dataclass, field
from io import StringIO
from pathlib import Path
from typing import TYPE_CHECKING, Any

from ruamel.yaml import YAML

from .context import get_flow
from .flow import FlowInfo
from .result import Ok, Result
from .task import TaskInfo

if TYPE_CHECKING:
    from .plan import TaskNode


# =============================================================================
# GHA Actions - Virtual tasks that map to `uses:` steps
# =============================================================================


class GHAAction:
    """
    A virtual task that represents a GitHub Actions `uses:` step.

    GHA actions are no-ops when run locally but generate `uses:` steps
    in workflow YAML. They can be called in flows just like regular tasks.

    Factory Functions vs Direct GHAAction:
        Most GHA actions are exposed through factory functions like
        `setup_python(version="3.11")` rather than direct GHAAction instances.
        These factories return GHAAction objects configured for the specific
        action. The returned GHAAction is callable (via `__call__`), which
        is what gets invoked in flows.

        - Factory: `setup_python(version="3.11")` returns a GHAAction
        - GHAAction is callable: `setup_python(version="3.11")()` or just
          `setup_python(version="3.11")` in a flow (auto-called)

    Example:
        @recompose.flow
        def build_pipeline(*, repo: str = "main") -> None:
            recompose.gha.checkout()  # checkout is a GHAAction instance
            recompose.gha.setup_python(version="3.11")  # factory returns GHAAction

            source = fetch_source(repo=repo)
            ...

    """

    def __init__(
        self,
        name: str,
        uses: str,
        *,
        with_params: dict[str, str] | None = None,
        doc: str | None = None,
    ):
        """
        Create a GHA action.

        Args:
            name: Display name for the action (e.g., "checkout", "setup_python")
            uses: The action reference (e.g., "actions/checkout@v4")
            with_params: Default `with:` parameters for the action
            doc: Documentation string

        """
        self.name = name
        self.uses = uses
        self.default_with_params = with_params or {}
        self.doc = doc

        # Create a TaskInfo for this action
        # The function is a no-op that returns Ok(None)
        def noop_fn(**kwargs: Any) -> Result[None]:
            return Ok(None)

        self._task_info = TaskInfo(
            name=f"gha.{name}",
            module="recompose.gha",
            fn=noop_fn,
            original_fn=noop_fn,
            signature=inspect.Signature(),  # Will be updated per-call
            doc=doc,
            is_gha_action=True,
            gha_uses=uses,
        )

    def __call__(self, **kwargs: Any) -> Result[None]:
        """
        Execute the action or add it to flow plan (context-aware).

        When called inside a @flow, adds a TaskNode to the plan.
        When called directly (not in a flow), this returns Ok(None) immediately (no-op locally).

        Args:
            **kwargs: Parameters to pass to the action (becomes `with:` in YAML)

        Returns:
            Result[None] when executed directly, TaskNode[None] when in a flow.

        """
        from .flow import get_current_plan
        from .plan import TaskNode

        plan = get_current_plan()

        if plan is not None:
            # FLOW-BUILDING MODE: Create TaskNode and add to plan
            # Merge default params with provided kwargs
            merged_params = {**self.default_with_params, **kwargs}

            # Create a TaskNode with the merged parameters
            node: TaskNode[None] = TaskNode(task_info=self._task_info, kwargs=merged_params)
            plan.add_node(node)
            return node  # type: ignore[return-value]

        # NORMAL EXECUTION MODE: No-op locally
        return Ok(None)


def _gha_action(
    name: str,
    uses: str,
    **default_params: str,
) -> GHAAction:
    """Helper to create a GHA action with default parameters."""
    return GHAAction(name, uses, with_params=default_params if default_params else None)


# =============================================================================
# Pre-defined GHA Actions
# =============================================================================

# Checkout repository
checkout = _gha_action(
    "checkout",
    "actions/checkout@v4",
)


# Setup Python
def setup_python(version: str = "3.11", **kwargs: Any) -> GHAAction:
    """
    Create a setup-python action with the specified version.

    Args:
        version: Python version to install (default: "3.11")
        **kwargs: Additional parameters for the action

    Returns:
        GHAAction that can be used in flows

    """
    return GHAAction(
        "setup_python",
        "actions/setup-python@v5",
        with_params={"python-version": version, **kwargs},
    )


# Setup uv
def setup_uv(version: str = "latest", **kwargs: Any) -> GHAAction:
    """
    Create a setup-uv action.

    Args:
        version: uv version to install (default: "latest")
        **kwargs: Additional parameters for the action

    Returns:
        GHAAction that can be used in flows

    """
    params = {**kwargs}
    if version != "latest":
        params["version"] = version
    return GHAAction(
        "setup_uv",
        "astral-sh/setup-uv@v4",
        with_params=params if params else None,
    )


# Setup Rust
def setup_rust(toolchain: str = "stable", **kwargs: Any) -> GHAAction:
    """
    Create a setup-rust action.

    Args:
        toolchain: Rust toolchain to install (default: "stable")
        **kwargs: Additional parameters for the action

    Returns:
        GHAAction that can be used in flows

    """
    return GHAAction(
        "setup_rust",
        "dtolnay/rust-toolchain@master",
        with_params={"toolchain": toolchain, **kwargs},
    )


# Cache
def cache(path: str, key: str, **kwargs: Any) -> GHAAction:
    """
    Create a cache action.

    Args:
        path: Path(s) to cache
        key: Cache key
        **kwargs: Additional parameters (e.g., restore-keys)

    Returns:
        GHAAction that can be used in flows

    """
    return GHAAction(
        "cache",
        "actions/cache@v4",
        with_params={"path": path, "key": key, **kwargs},
    )


# =============================================================================
# Workflow Spec Dataclasses
# =============================================================================


@dataclass
class StepSpec:
    """A step within a GHA job."""

    name: str
    run: str | None = None
    uses: str | None = None
    with_: dict[str, Any] | None = None
    env: dict[str, str] | None = None
    id: str | None = None
    if_condition: str | None = None  # GHA `if:` expression
    comment: str | None = None  # Comment to add above the step (for run_if conditions)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        from ruamel.yaml.comments import CommentedMap

        d: CommentedMap = CommentedMap()
        d["name"] = self.name
        if self.id:
            d["id"] = self.id
        if self.if_condition:
            d["if"] = self.if_condition
        if self.uses:
            d["uses"] = self.uses
        if self.with_:
            d["with"] = self.with_
        if self.run:
            d["run"] = self.run
        if self.env:
            d["env"] = self.env

        # Add comment before the 'run' key if specified
        if self.comment and self.run:
            # indent=6 aligns with step keys when nested under jobs.<name>.steps list
            d.yaml_set_comment_before_after_key("run", before=self.comment, indent=6)

        return d


@dataclass
class JobSpec:
    """A job within a GHA workflow."""

    name: str
    runs_on: str = "ubuntu-latest"
    steps: list[StepSpec] = field(default_factory=list)
    env: dict[str, str] | None = None
    timeout_minutes: int | None = None
    working_directory: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        d: dict[str, Any] = {"runs-on": self.runs_on}
        if self.working_directory:
            d["defaults"] = {"run": {"working-directory": self.working_directory}}
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
    default: str | bool | int | float | None = None
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


def generate_workflow_header(source: str | None = None) -> str:
    """
    Generate a header comment for generated workflow files.

    Args:
        source: Optional description of what generated this workflow
                (e.g., "flow: ci" or "automation: nightly")

    Returns:
        Header comment string to prepend to YAML content.

    """
    lines = [
        "# ============================================================================",
        "# GENERATED FILE - DO NOT EDIT MANUALLY",
        "#",
        "# This workflow is generated by recompose. To modify:",
        "#   1. Edit the source flow/automation definition",
        "#   2. Run: ./run generate_gha",
        "#   3. Commit the regenerated file",
        "#",
    ]
    if source:
        lines.append(f"# Source: {source}")
    lines.extend(
        [
            "# ============================================================================",
            "",
        ]
    )
    return "\n".join(lines)


@dataclass
class WorkflowSpec:
    """A complete GHA workflow."""

    name: str
    on: dict[str, Any]
    jobs: dict[str, JobSpec]
    path: Path | None = None  # Output file path, if known

    def __str__(self) -> str:
        """User-friendly string representation."""
        num_jobs = len(self.jobs)
        total_steps = sum(len(job.steps) for job in self.jobs.values())
        triggers = ", ".join(self.on.keys())
        path_str = f" -> {self.path}" if self.path else ""
        return f"WorkflowSpec({self.name}) - {num_jobs} job(s), {total_steps} step(s), on: {triggers}{path_str}"

    __repr__ = __str__

    def to_dict(self) -> dict[str, Any]:
        """Convert to dict for YAML serialization."""
        return {
            "name": self.name,
            "on": self.on,
            "jobs": {name: job.to_dict() for name, job in self.jobs.items()},
        }

    def to_yaml(self, *, include_header: bool = False, source: str | None = None) -> str:
        """
        Render as YAML string.

        Args:
            include_header: If True, prepend generated-file header comment.
            source: Source description for header (e.g., "flow: ci").
                    If not provided and include_header=True, uses workflow name.

        Returns:
            YAML string, optionally with header.

        """
        yaml = YAML()
        yaml.default_flow_style = False

        stream = StringIO()
        yaml.dump(self.to_dict(), stream)
        yaml_content = stream.getvalue()

        if include_header:
            header_source = source if source else f"workflow: {self.name}"
            return generate_workflow_header(header_source) + yaml_content

        return yaml_content


def _python_type_to_gha_input_type(annotation: Any) -> str:
    """Map Python type annotation to GHA input type."""
    if annotation is bool:
        return "boolean"
    if annotation is int or annotation is float:
        return "number"
    # Default to string for str, Path, and anything else
    return "string"


def _default_to_gha_value(value: Any) -> str | bool | int | float | None:
    """Convert a Python default value to appropriate type for GHA input."""
    if value is None or value is inspect.Parameter.empty:
        return None
    if isinstance(value, bool):
        # GHA boolean inputs need actual boolean defaults, not strings
        return value
    if isinstance(value, (int, float)):
        return value
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
        default_value = _default_to_gha_value(param.default) if has_default else None

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




def _build_setup_step(step_name: str, flow_info: FlowInfo, script_path: str, python_cmd: str) -> StepSpec:
    """Build the setup step that initializes the workspace."""
    inputs = _flow_params_to_inputs(flow_info)

    # Build the run command with all input parameters
    # Note: workspace is set via RECOMPOSE_WORKSPACE env var at job level
    cmd_parts = [
        python_cmd,
        script_path,
        flow_info.name,
        "--setup",
    ]

    # Add each parameter from workflow_dispatch inputs
    for inp in inputs:
        cmd_parts.append(f"--{inp.name}")
        cmd_parts.append(f"${{{{ inputs.{inp.name} }}}}")

    return StepSpec(
        name=step_name,
        run=" ".join(cmd_parts),
    )


def _build_task_step(
    step_name: str,
    flow_name: str,
    script_path: str,
    python_cmd: str,
    condition_check_step: str | None = None,
) -> StepSpec:
    """Build a step that executes a single task."""
    # Note: workspace is set via RECOMPOSE_WORKSPACE env var at job level
    if_condition = None
    if condition_check_step:
        # Reference the condition-check step's output
        if_condition = f"${{{{ steps.{condition_check_step}.outputs.value == 'true' }}}}"

    return StepSpec(
        name=step_name,
        run=f"{python_cmd} {script_path} {flow_name} --step {step_name}",
        if_condition=if_condition,
    )


def _build_gha_action_step(step_name: str, node: Any) -> StepSpec:
    """Build a step for a GHA action (uses: instead of run:)."""
    task_info = node.task_info
    uses = task_info.gha_uses

    # Get with: parameters from node kwargs
    with_params = node.kwargs if node.kwargs else None

    return StepSpec(
        name=step_name,
        uses=uses,
        with_=with_params,
    )


def _build_condition_check_step(
    step_name: str,
    flow_name: str,
    script_path: str,
    python_cmd: str,
    condition_expr_str: str,
) -> StepSpec:
    """Build a step that evaluates a condition and outputs true/false."""
    return StepSpec(
        name=step_name,
        id=step_name,  # Need ID for referencing in if: conditions
        run=f"{python_cmd} {script_path} {flow_name} --step {step_name}",
        comment=f"[if: {condition_expr_str}]",
    )


def render_flow_workflow(
    flow_info: FlowInfo,
    script_path: str = "app.py",
    runs_on: str = "ubuntu-latest",
    python_cmd: str = "python",
    working_directory: str | None = None,
) -> WorkflowSpec:
    """
    Generate a WorkflowSpec from a flow.

    Args:
        flow_info: The flow to generate a workflow for.
        script_path: Path to the script that contains the flow (relative to repo root).
        runs_on: The runner to use for the job.
        python_cmd: Command to invoke Python (e.g., "python", "uv run python").
        working_directory: Working directory for run steps (relative to repo root).

    Returns:
        A WorkflowSpec that can be rendered to YAML.

    """
    from .expr import format_expr

    # Build workflow_dispatch inputs from flow parameters
    inputs = _flow_params_to_inputs(flow_info)
    inputs_dict = {inp.name: inp.to_dict() for inp in inputs}

    # Build the on trigger
    on_trigger: dict[str, Any] = {"workflow_dispatch": {}}
    if inputs_dict:
        on_trigger["workflow_dispatch"]["inputs"] = inputs_dict

    # Use the pre-built plan (built at decoration time with InputPlaceholders)
    # Condition-check nodes are already first-class nodes in the plan
    plan = flow_info.plan

    # Separate GHA actions from regular tasks (including condition-check nodes)
    gha_nodes = [n for n in plan.nodes if n.task_info.is_gha_action]
    non_gha_nodes = [n for n in plan.nodes if not n.task_info.is_gha_action]
    has_regular_tasks = any(not n.task_info.is_condition_check for n in non_gha_nodes)
    has_gha_actions = len(gha_nodes) > 0

    # Build job steps
    job_steps: list[StepSpec] = []

    # If no GHA actions in flow, add checkout automatically for convenience
    if not has_gha_actions:
        job_steps.append(
            StepSpec(
                name="Checkout",
                uses="actions/checkout@v4",
            )
        )

    # Add GHA action steps first (in order)
    for node in gha_nodes:
        step_name = node.step_name or node.name
        job_steps.append(_build_gha_action_step(step_name, node))

    # Add setup step if there are regular tasks
    if has_regular_tasks:
        job_steps.append(_build_setup_step("setup_workspace", flow_info, script_path, python_cmd))

    # Add non-GHA steps in order (condition-checks and regular tasks)
    # The plan already has condition-check nodes interleaved correctly
    for node in non_gha_nodes:
        step_name = node.step_name or node.name

        if node.task_info.is_condition_check:
            # Render the condition-check step
            # Extract the condition expression for the comment
            condition_data = node.kwargs.get("condition_data", {})
            condition_expr_str = format_expr(condition_data) if condition_data else "unknown"
            job_steps.append(
                _build_condition_check_step(
                    step_name,
                    flow_info.name,
                    script_path,
                    python_cmd,
                    condition_expr_str,
                )
            )
        else:
            # Regular task step - use condition_check_step reference if present
            job_steps.append(
                _build_task_step(
                    step_name,
                    flow_info.name,
                    script_path,
                    python_cmd,
                    condition_check_step=node.condition_check_step,
                )
            )

    # Build the job
    job = JobSpec(
        name=flow_info.name,
        runs_on=runs_on,
        steps=job_steps,
        working_directory=working_directory,
        env={"RECOMPOSE_WORKSPACE": ".recompose"},
    )

    # Build the workflow
    return WorkflowSpec(
        name=flow_info.name,
        on=on_trigger,
        jobs={flow_info.name: job},
    )


def render_automation_workflow(
    automation_info: Any,  # AutomationInfo, but avoid circular import
) -> WorkflowSpec:
    """
    Generate a WorkflowSpec from an automation.

    Automations dispatch flows via workflow_dispatch. The generated workflow
    contains steps that use `gh workflow run` to trigger child flows.

    Args:
        automation_info: The automation to generate a workflow for.

    Returns:
        A WorkflowSpec that can be rendered to YAML.

    """
    # Build the plan to get dispatches
    plan = automation_info.fn.plan()

    # Determine the trigger
    if automation_info.gha_on:
        on_trigger = automation_info.gha_on
    else:
        # Default to workflow_dispatch if no trigger specified
        on_trigger = {"workflow_dispatch": {}}

    # Build job steps
    job_steps: list[StepSpec] = []

    # Add checkout (needed for gh CLI authentication in some cases)
    job_steps.append(
        StepSpec(
            name="Checkout",
            uses="actions/checkout@v4",
        )
    )

    # Add a step for each flow dispatch
    for i, dispatch in enumerate(plan.dispatches, 1):
        # Build the gh workflow run command
        workflow_file = f"{dispatch.flow_name}.yml"

        # Build inputs JSON if there are params
        if dispatch.params:
            import json

            inputs_json = json.dumps(dispatch.params)
            run_cmd = f"gh workflow run {workflow_file} --json <<< '{inputs_json}'"
        else:
            run_cmd = f"gh workflow run {workflow_file}"

        job_steps.append(
            StepSpec(
                name=f"Dispatch {dispatch.flow_name}",
                run=run_cmd,
                env={"GH_TOKEN": "${{ secrets.GITHUB_TOKEN }}"},
            )
        )

    # Build the job
    job = JobSpec(
        name=automation_info.name,
        runs_on=automation_info.gha_runs_on,
        steps=job_steps,
        env=automation_info.gha_env,
        timeout_minutes=automation_info.gha_timeout_minutes,
    )

    # Build the workflow
    return WorkflowSpec(
        name=automation_info.name,
        on=on_trigger,
        jobs={automation_info.name: job},
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
