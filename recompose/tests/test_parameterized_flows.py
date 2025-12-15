"""Tests for parameterized flows in GHA generation.

This test file validates the behavior of flows with required (no default) parameters
when generating GitHub Actions workflows.
"""

import pytest
from ruamel.yaml import YAML

import recompose
from recompose.flowgraph import InputPlaceholder
from recompose.gha import render_flow_workflow


# Test tasks
@recompose.task
def greet(*, name: str) -> recompose.Result[str]:
    """A task that greets someone."""
    return recompose.Ok(f"Hello, {name}!")


@recompose.task
def count(*, n: int = 10) -> recompose.Result[int]:
    """A task that counts."""
    return recompose.Ok(n)


@recompose.task
def echo(*, message: str) -> recompose.Result[str]:
    """A task that echoes a message."""
    return recompose.Ok(message)


# Flow with REQUIRED parameter (no default)
@recompose.flow
def flow_with_required_param(*, name: str) -> None:
    """A flow that requires a name parameter."""
    greet.flow(name=name)


# Flow with mix of required and optional parameters
@recompose.flow
def flow_with_mixed_params(*, name: str, count_to: int = 10) -> None:
    """A flow with both required and optional parameters."""
    greet.flow(name=name)
    count.flow(n=count_to)


# Flow that passes required param to multiple tasks
@recompose.flow
def flow_with_param_reuse(*, message: str) -> None:
    """A flow that uses the same param in multiple tasks."""
    echo.flow(message=message)
    echo.flow(message=message)


class TestFlowsWithRequiredParams:
    """Tests for flows that have required parameters (no defaults)."""

    def test_flow_with_required_param_works_with_value(self) -> None:
        """Test that a flow with required params works when given values."""
        result = flow_with_required_param(name="World")
        assert result.ok

    def test_flow_with_required_param_plan_works_with_value(self) -> None:
        """Test that .plan() works when given required params."""
        plan = flow_with_required_param.plan(name="World")
        assert len(plan.nodes) == 1
        assert plan.nodes[0].task_info.name == "greet"

    def test_flow_with_required_param_gha_generation(self) -> None:
        """Test that GHA generation works for flows with required params."""
        flow_info = recompose.get_flow("flow_with_required_param")
        assert flow_info is not None

        # This should work - the workflow should accept 'name' as a workflow_dispatch input
        spec = render_flow_workflow(flow_info, script_path="app.py")

        # Check that the workflow_dispatch input is created correctly
        assert "workflow_dispatch" in spec.on
        inputs = spec.on["workflow_dispatch"].get("inputs", {})
        assert "name" in inputs
        assert inputs["name"]["required"] is True

        # Check that the setup step references the input
        job = spec.jobs["flow_with_required_param"]
        setup_step = next((s for s in job.steps if "setup_workspace" in s.name), None)
        assert setup_step is not None
        assert "${{ inputs.name }}" in (setup_step.run or "")

    def test_flow_with_mixed_params_gha_generation(self) -> None:
        """Test GHA generation for flows with both required and optional params."""
        flow_info = recompose.get_flow("flow_with_mixed_params")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        inputs = spec.on["workflow_dispatch"].get("inputs", {})

        # Required param
        assert "name" in inputs
        assert inputs["name"]["required"] is True
        assert "default" not in inputs["name"]

        # Optional param
        assert "count_to" in inputs
        assert inputs["count_to"]["required"] is False
        assert inputs["count_to"]["default"] == "10"


class TestInputPlaceholder:
    """Tests for the InputPlaceholder class."""

    def test_input_placeholder_str(self) -> None:
        """Test that InputPlaceholder.__str__ returns GHA input reference format."""
        placeholder = InputPlaceholder[str](name="repo")
        assert str(placeholder) == "${{ inputs.repo }}"

    def test_input_placeholder_repr(self) -> None:
        """Test InputPlaceholder repr."""
        placeholder = InputPlaceholder[str](name="repo", annotation=str)
        assert "InputPlaceholder(repo: str)" == repr(placeholder)

    def test_input_placeholder_in_flow_plan(self) -> None:
        """Test that InputPlaceholder is stored in TaskNode kwargs during plan construction."""
        # Create a placeholder like GHA generation does
        placeholder = InputPlaceholder[str](name="name", annotation=str)

        # Build the plan with the placeholder
        plan = flow_with_required_param.plan(name=placeholder)

        # The TaskNode should have the placeholder in its kwargs
        assert len(plan.nodes) == 1
        node = plan.nodes[0]
        assert "name" in node.kwargs
        assert isinstance(node.kwargs["name"], InputPlaceholder)
        assert node.kwargs["name"].name == "name"

    def test_input_placeholder_reused_across_tasks(self) -> None:
        """Test that the same InputPlaceholder can be used in multiple tasks."""
        placeholder = InputPlaceholder[str](name="message", annotation=str)

        plan = flow_with_param_reuse.plan(message=placeholder)

        # Both tasks should have the same placeholder
        assert len(plan.nodes) == 2
        for node in plan.nodes:
            assert isinstance(node.kwargs["message"], InputPlaceholder)
            assert node.kwargs["message"].name == "message"


class TestParameterizedFlowYamlOutput:
    """Tests for the YAML output of parameterized flows."""

    def test_yaml_is_valid(self) -> None:
        """Test that generated YAML for flows with required params is valid."""
        flow_info = recompose.get_flow("flow_with_required_param")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "flow_with_required_param"

    def test_yaml_has_correct_input_structure(self) -> None:
        """Test that the YAML has correct workflow_dispatch input structure."""
        flow_info = recompose.get_flow("flow_with_required_param")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        yaml = YAML()
        parsed = yaml.load(yaml_str)

        # Check the on trigger has workflow_dispatch with inputs
        assert "workflow_dispatch" in parsed["on"]
        assert "inputs" in parsed["on"]["workflow_dispatch"]
        assert "name" in parsed["on"]["workflow_dispatch"]["inputs"]

        name_input = parsed["on"]["workflow_dispatch"]["inputs"]["name"]
        assert name_input["required"] is True
        assert name_input["type"] == "string"

    def test_setup_step_passes_inputs_correctly(self) -> None:
        """Test that the setup step in YAML correctly passes workflow inputs."""
        flow_info = recompose.get_flow("flow_with_mixed_params")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        yaml = YAML()
        parsed = yaml.load(yaml_str)

        # Find the setup step
        steps = parsed["jobs"]["flow_with_mixed_params"]["steps"]
        setup_step = next((s for s in steps if "setup_workspace" in s["name"]), None)
        assert setup_step is not None

        # The run command should include both inputs
        run_cmd = setup_step["run"]
        assert "--name ${{ inputs.name }}" in run_cmd
        assert "--count_to ${{ inputs.count_to }}" in run_cmd


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
