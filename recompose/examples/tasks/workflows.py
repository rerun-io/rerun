"""
Workflow generation and validation tasks for recompose.

These tasks manage GitHub Actions workflow files:
- update_workflows: Regenerates workflow files from flows (local dev task)
- validate_workflows: Checks generated matches committed (CI task)
"""

from pathlib import Path

import recompose
from recompose.gha import render_flow_workflow

# Project paths
PROJECT_ROOT = Path(__file__).parent.parent.parent  # recompose/
RERUN_ROOT = PROJECT_ROOT.parent  # rerun/
WORKFLOWS_DIR = RERUN_ROOT / ".github" / "workflows"


def _get_workflow_configs() -> list[dict]:
    """
    Return the list of workflows to generate.

    Each config specifies:
    - flow_name: Name of the flow to generate from
    - output_file: Name of the output YAML file
    - script_path: Path to the script (relative to repo root)
    """
    return [
        {
            "flow_name": "ci",
            "output_file": "recompose_ci.yml",
            "script_path": "recompose/run",
        },
    ]


def _generate_workflow(flow_name: str, script_path: str) -> str:
    """Generate workflow YAML for a flow, with header."""
    from recompose.flow import get_flow

    flow_info = get_flow(flow_name)
    if flow_info is None:
        raise ValueError(f"Flow '{flow_name}' not found")

    spec = render_flow_workflow(flow_info, script_path=script_path)
    return spec.to_yaml(include_header=True, source=f"flow: {flow_name}")


@recompose.task
def update_workflows() -> recompose.Result[None]:
    """
    Regenerate GitHub Actions workflow files from flows.

    This is a local development task. Run after modifying flow definitions
    to update the committed workflow files.

    Writes to: .github/workflows/recompose_*.yml
    """
    configs = _get_workflow_configs()

    recompose.out(f"Generating {len(configs)} workflow(s)...")

    for config in configs:
        flow_name = config["flow_name"]
        output_file = config["output_file"]
        script_path = config["script_path"]

        recompose.out(f"  {flow_name} -> {output_file}")

        try:
            yaml_content = _generate_workflow(flow_name, script_path)
        except Exception as e:
            return recompose.Err(f"Failed to generate {flow_name}: {e}")

        output_path = WORKFLOWS_DIR / output_file
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(yaml_content)

    recompose.out(f"Wrote workflows to {WORKFLOWS_DIR}")
    return recompose.Ok(None)


@recompose.task
def validate_workflows() -> recompose.Result[None]:
    """
    Validate that committed workflows match generated ones.

    This runs in CI to ensure workflow files haven't been manually edited
    and are in sync with the flow definitions.

    Fails if any workflow file differs from what would be generated.
    """
    configs = _get_workflow_configs()
    errors: list[str] = []

    recompose.out(f"Validating {len(configs)} workflow(s)...")

    for config in configs:
        flow_name = config["flow_name"]
        output_file = config["output_file"]
        script_path = config["script_path"]
        output_path = WORKFLOWS_DIR / output_file

        recompose.out(f"  Checking {output_file}...")

        # Generate expected content
        try:
            expected = _generate_workflow(flow_name, script_path)
        except Exception as e:
            errors.append(f"{output_file}: Failed to generate - {e}")
            continue

        # Check if file exists
        if not output_path.exists():
            errors.append(f"{output_file}: File does not exist (run update_workflows)")
            continue

        # Compare content
        actual = output_path.read_text()
        if actual != expected:
            errors.append(f"{output_file}: Content differs from generated (run update_workflows and commit the result)")
            continue

        recompose.out(f"    {output_file} OK")

    if errors:
        recompose.out("")
        for error in errors:
            recompose.out(f"  ERROR: {error}")
        return recompose.Err(f"Workflow validation failed: {len(errors)} error(s)")

    recompose.out("All workflows are in sync!")
    return recompose.Ok(None)
