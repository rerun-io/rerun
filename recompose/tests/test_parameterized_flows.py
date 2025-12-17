"""Tests for parameterized flows in GHA generation.

This test file validates the behavior of flows with required (no default) parameters
when generating GitHub Actions workflows.
"""

import pytest
from ruamel.yaml import YAML

import recompose
from recompose.gha import render_flow_workflow
from recompose.plan import InputPlaceholder

from . import flow_test_app

# Import flows from test app for execution tests
flow_with_required_param = flow_test_app.flow_with_required_param
flow_with_mixed_params = flow_test_app.flow_with_mixed_params
flow_with_param_reuse = flow_test_app.flow_with_param_reuse

# Import tasks for plan-only tests (these don't need subprocess)
greet = flow_test_app.greet
count_task = flow_test_app.count_task
echo = flow_test_app.echo


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
        flow_info = flow_with_required_param._flow_info

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
        flow_info = flow_with_mixed_params._flow_info

        spec = render_flow_workflow(flow_info, script_path="app.py")

        inputs = spec.on["workflow_dispatch"].get("inputs", {})

        # Required param
        assert "name" in inputs
        assert inputs["name"]["required"] is True
        assert "default" not in inputs["name"]

        # Optional param
        assert "count_to" in inputs
        assert inputs["count_to"]["required"] is False
        assert inputs["count_to"]["default"] == 10  # GHA number inputs preserve actual int type


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


class TestInputTypeAlias:
    """Tests for the Input[T] type alias."""

    def test_input_type_alias_exists(self) -> None:
        """Test that Input is exported from recompose."""
        from recompose import Input

        # Input[str] should be a Union type
        input_str = Input[str]
        assert "Union" in str(input_str) or "str" in str(input_str)

    def test_input_type_alias_components(self) -> None:
        """Test that Input[T] includes the expected component types."""
        from typing import get_args

        from recompose import Input

        args = get_args(Input[str])
        arg_names = [str(a) for a in args]

        # Should include str, TaskNode[str], InputPlaceholder[str]
        assert any("str" in name and "TaskNode" not in name and "InputPlaceholder" not in name for name in arg_names)
        assert any("TaskNode" in name for name in arg_names)
        assert any("InputPlaceholder" in name for name in arg_names)


class TestTaskSignature:
    """Tests for task signature and validation."""

    def test_task_has_signature(self) -> None:
        """Test that task has __signature__ from original function."""
        import inspect

        sig = inspect.signature(greet)
        param_names = list(sig.parameters.keys())
        assert "name" in param_names

    def test_task_rejects_unknown_kwargs_in_flow(self) -> None:
        """Test that task raises TypeError for unknown kwargs when called in flow."""

        @recompose.flow
        def test_flow() -> None:
            # This should raise TypeError for unknown kwarg
            greet(name="test", unknown_arg="bad")  # type: ignore[call-arg]

        with pytest.raises(TypeError, match="unexpected keyword argument"):
            test_flow()

    def test_flow_method_rejects_missing_required(self) -> None:
        """Test that () raises TypeError for missing required args."""

        @recompose.flow
        def test_flow() -> None:
            # greet requires 'name' parameter
            greet()  # type: ignore[call-arg]

        with pytest.raises(TypeError, match="missing required keyword argument"):
            test_flow()

    def test_flow_method_accepts_optional_missing(self) -> None:
        """Test that () accepts missing optional args."""
        # Use the flow from test app that exercises optional params
        result = flow_test_app.flow_with_optional_only()
        assert result.ok

    def test_flow_method_accepts_task_node_as_value(self) -> None:
        """Test that () accepts TaskNode from another () call."""
        # Use the flow from test app that exercises .value() composition
        result = flow_test_app.flow_with_value_composition()
        assert result.ok

    def test_flow_method_accepts_input_placeholder(self) -> None:
        """Test that () accepts InputPlaceholder values."""

        @recompose.flow
        def test_flow(*, name: str) -> None:
            greet(name=name)

        # Build plan with placeholder
        placeholder = InputPlaceholder[str](name="name")
        plan = test_flow.plan(name=placeholder)

        assert len(plan.nodes) == 1
        assert plan.nodes[0].kwargs["name"] is placeholder


class TestValueBasedComposition:
    """Tests for the type-safe .value() pattern in flow composition."""

    def test_task_node_has_value_method(self) -> None:
        """Test that TaskNode has a .value() method that returns itself."""
        import inspect

        from recompose.plan import TaskNode
        from recompose.task import TaskInfo

        # Create a mock TaskInfo
        def dummy_fn() -> recompose.Result[str]:
            return recompose.Ok("test")

        info = TaskInfo(
            name="dummy",
            module="test",
            fn=dummy_fn,
            original_fn=dummy_fn,
            signature=inspect.signature(dummy_fn),
            doc=None,
        )

        node: TaskNode[str] = TaskNode(task_info=info, kwargs={})

        # .value() should return the node itself
        assert node.value() is node

    def test_task_node_mimics_result_interface(self) -> None:
        """Test that TaskNode has ok, failed, error properties like Result."""
        import inspect

        from recompose.plan import TaskNode
        from recompose.task import TaskInfo

        def dummy_fn() -> recompose.Result[str]:
            return recompose.Ok("test")

        info = TaskInfo(
            name="dummy",
            module="test",
            fn=dummy_fn,
            original_fn=dummy_fn,
            signature=inspect.signature(dummy_fn),
            doc=None,
        )

        node: TaskNode[str] = TaskNode(task_info=info, kwargs={})

        # Should mimic a successful Result
        assert node.ok is True
        assert node.failed is False
        assert node.error is None

    def test_input_placeholder_has_value_method(self) -> None:
        """Test that InputPlaceholder has a .value() method that returns itself."""
        placeholder = InputPlaceholder[str](name="test")

        # .value() should return the placeholder itself
        assert placeholder.value() is placeholder

    def test_flow_composition_with_value(self) -> None:
        """Test the type-safe .value() pattern for flow composition."""
        # Use the flow from test app that exercises .value() composition
        result = flow_test_app.flow_with_value_composition()
        assert result.ok

    def test_flow_plan_tracks_value_dependencies(self) -> None:
        """Test that using .value() creates proper dependencies in the plan."""

        @recompose.flow
        def test_flow() -> None:
            result = greet(name="World")
            echo(message=result.value())

        plan = test_flow.plan()

        # Should have 2 nodes
        assert len(plan.nodes) == 2

        # Second node should depend on first
        greet_node = plan.nodes[0]
        echo_node = plan.nodes[1]

        assert greet_node.task_info.name == "greet"
        assert echo_node.task_info.name == "echo"

        # The echo node's kwargs should contain the greet node (via .value())
        assert echo_node.kwargs["message"] is greet_node

    def test_flow_plan_with_placeholder_value(self) -> None:
        """Test that InputPlaceholder.value() works in flow composition."""

        @recompose.flow
        def test_flow(*, name: str) -> None:
            greet(name=name)

        # Build plan with placeholder - simulating GHA generation
        placeholder = InputPlaceholder[str](name="name")
        plan = test_flow.plan(name=placeholder)

        # The placeholder should be in the node's kwargs
        assert plan.nodes[0].kwargs["name"] is placeholder


class TestParameterizedFlowYamlOutput:
    """Tests for the YAML output of parameterized flows."""

    def test_yaml_is_valid(self) -> None:
        """Test that generated YAML for flows with required params is valid."""
        flow_info = flow_with_required_param._flow_info

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "flow_with_required_param"

    def test_yaml_has_correct_input_structure(self) -> None:
        """Test that the YAML has correct workflow_dispatch input structure."""
        flow_info = flow_with_required_param._flow_info

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
        flow_info = flow_with_mixed_params._flow_info

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
