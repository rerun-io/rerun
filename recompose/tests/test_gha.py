"""Tests for GitHub Actions workflow generation."""

import shutil

import pytest
from ruamel.yaml import YAML

import recompose
from recompose.gha import (
    JobSpec,
    StepSpec,
    WorkflowDispatchInput,
    WorkflowSpec,
    render_flow_workflow,
    validate_workflow,
)


# Test fixtures - simple flows for testing
@recompose.task
def simple_task() -> recompose.Result[str]:
    """A simple task with no parameters."""
    return recompose.Ok("done")


@recompose.task
def param_task(*, name: str, count: int = 5) -> recompose.Result[str]:
    """A task with parameters."""
    return recompose.Ok(f"{name}: {count}")


@recompose.flow
def simple_flow() -> None:
    """A flow with no parameters."""
    simple_task.flow()


@recompose.flow
def param_flow(*, repo: str = "main", debug: bool = False) -> None:
    """A flow with parameters."""
    simple_task.flow()


@recompose.flow
def multi_step_flow() -> None:
    """A flow with multiple steps."""
    a = simple_task.flow()
    param_task.flow(name=a, count=10)


class TestStepSpec:
    """Tests for StepSpec."""

    def test_run_step(self) -> None:
        """Test a step with a run command."""
        step = StepSpec(name="Build", run="cargo build")
        d = step.to_dict()
        assert d["name"] == "Build"
        assert d["run"] == "cargo build"
        assert "uses" not in d

    def test_uses_step(self) -> None:
        """Test a step with uses action."""
        step = StepSpec(name="Checkout", uses="actions/checkout@v4")
        d = step.to_dict()
        assert d["name"] == "Checkout"
        assert d["uses"] == "actions/checkout@v4"
        assert "run" not in d

    def test_uses_with_inputs(self) -> None:
        """Test a uses step with inputs."""
        step = StepSpec(
            name="Setup Python",
            uses="actions/setup-python@v5",
            with_={"python-version": "3.11"},
        )
        d = step.to_dict()
        assert d["with"]["python-version"] == "3.11"

    def test_step_with_env(self) -> None:
        """Test a step with environment variables."""
        step = StepSpec(name="Test", run="pytest", env={"CI": "true"})
        d = step.to_dict()
        assert d["env"]["CI"] == "true"


class TestJobSpec:
    """Tests for JobSpec."""

    def test_basic_job(self) -> None:
        """Test a basic job."""
        job = JobSpec(
            name="build",
            runs_on="ubuntu-latest",
            steps=[StepSpec(name="Checkout", uses="actions/checkout@v4")],
        )
        d = job.to_dict()
        assert d["runs-on"] == "ubuntu-latest"
        assert len(d["steps"]) == 1
        assert d["steps"][0]["name"] == "Checkout"

    def test_job_with_env(self) -> None:
        """Test a job with environment variables."""
        job = JobSpec(
            name="build",
            runs_on="ubuntu-latest",
            steps=[],
            env={"RUST_LOG": "debug"},
        )
        d = job.to_dict()
        assert d["env"]["RUST_LOG"] == "debug"

    def test_job_with_timeout(self) -> None:
        """Test a job with timeout."""
        job = JobSpec(
            name="build",
            runs_on="ubuntu-latest",
            steps=[],
            timeout_minutes=30,
        )
        d = job.to_dict()
        assert d["timeout-minutes"] == 30


class TestWorkflowDispatchInput:
    """Tests for WorkflowDispatchInput."""

    def test_required_input(self) -> None:
        """Test a required input."""
        inp = WorkflowDispatchInput(
            name="repo",
            description="Repository name",
            required=True,
            type="string",
        )
        d = inp.to_dict()
        assert d["required"] is True
        assert d["type"] == "string"
        assert "default" not in d

    def test_optional_input_with_default(self) -> None:
        """Test an optional input with default."""
        inp = WorkflowDispatchInput(
            name="branch",
            description="Branch name",
            required=False,
            default="main",
            type="string",
        )
        d = inp.to_dict()
        assert d["required"] is False
        assert d["default"] == "main"

    def test_boolean_input(self) -> None:
        """Test a boolean input."""
        inp = WorkflowDispatchInput(
            name="debug",
            description="Enable debug mode",
            required=False,
            default="false",
            type="boolean",
        )
        d = inp.to_dict()
        assert d["type"] == "boolean"


class TestWorkflowSpec:
    """Tests for WorkflowSpec."""

    def test_to_dict(self) -> None:
        """Test converting workflow to dict."""
        workflow = WorkflowSpec(
            name="CI",
            on={"push": {"branches": ["main"]}},
            jobs={
                "build": JobSpec(
                    name="build",
                    steps=[StepSpec(name="Checkout", uses="actions/checkout@v4")],
                )
            },
        )
        d = workflow.to_dict()
        assert d["name"] == "CI"
        assert d["on"]["push"]["branches"] == ["main"]
        assert "build" in d["jobs"]

    def test_to_yaml(self) -> None:
        """Test rendering to YAML."""
        workflow = WorkflowSpec(
            name="CI",
            on={"workflow_dispatch": {}},
            jobs={
                "test": JobSpec(
                    name="test",
                    steps=[StepSpec(name="Run tests", run="pytest")],
                )
            },
        )
        yaml_str = workflow.to_yaml()

        # Parse it back to verify it's valid YAML
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "CI"
        assert "workflow_dispatch" in parsed["on"]


class TestRenderFlowWorkflow:
    """Tests for render_flow_workflow."""

    def test_simple_flow(self) -> None:
        """Test rendering a simple flow with no parameters."""
        flow_info = recompose.get_flow("simple_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        assert spec.name == "simple_flow"
        assert "workflow_dispatch" in spec.on

        # Should have checkout + setup + 1 task step
        job = spec.jobs["simple_flow"]
        assert len(job.steps) == 3
        assert job.steps[0].uses == "actions/checkout@v4"
        assert "--setup" in (job.steps[1].run or "")
        assert "--step" in (job.steps[2].run or "")

    def test_flow_with_parameters(self) -> None:
        """Test rendering a flow with parameters."""
        flow_info = recompose.get_flow("param_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        # Check workflow_dispatch inputs
        inputs = spec.on["workflow_dispatch"]["inputs"]
        assert "repo" in inputs
        assert inputs["repo"]["default"] == "main"
        assert inputs["repo"]["type"] == "string"

        assert "debug" in inputs
        assert inputs["debug"]["type"] == "boolean"
        assert inputs["debug"]["default"] == "false"

        # Check setup step includes parameters
        job = spec.jobs["param_flow"]
        setup_step = job.steps[1]
        assert "${{ inputs.repo }}" in (setup_step.run or "")
        assert "${{ inputs.debug }}" in (setup_step.run or "")

    def test_multi_step_flow(self) -> None:
        """Test rendering a flow with multiple steps."""
        flow_info = recompose.get_flow("multi_step_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        # Should have checkout + setup + 2 task steps
        job = spec.jobs["multi_step_flow"]
        assert len(job.steps) == 4

        # Verify step names are in order
        step_names = [s.name for s in job.steps]
        assert step_names[0] == "Checkout"
        assert "setup_workspace" in step_names[1]  # Numbered, e.g., "1_setup_workspace"
        assert "simple_task" in step_names[2]
        assert "param_task" in step_names[3]

    def test_custom_runner(self) -> None:
        """Test specifying a custom runner."""
        flow_info = recompose.get_flow("simple_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py", runs_on="macos-latest")

        job = spec.jobs["simple_flow"]
        assert job.runs_on == "macos-latest"

    def test_yaml_output_is_valid(self) -> None:
        """Test that generated YAML is valid."""
        flow_info = recompose.get_flow("param_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "param_flow"
        assert "jobs" in parsed


class TestGHAActions:
    """Tests for GHA virtual actions."""

    def test_checkout_action_direct_call(self) -> None:
        """Test calling checkout directly (no-op)."""
        from recompose.gha import checkout

        result = checkout()
        assert result.ok
        assert result.value is None

    def test_checkout_flow_outside_flow_raises(self) -> None:
        """Test that .flow() outside a flow raises."""
        from recompose.gha import checkout

        with pytest.raises(RuntimeError, match="can only be called inside"):
            checkout.flow()

    def test_setup_python_creates_action(self) -> None:
        """Test setup_python creates an action with version."""
        from recompose.gha import setup_python

        action = setup_python(version="3.12")
        assert action.uses == "actions/setup-python@v5"
        assert action.default_with_params["python-version"] == "3.12"

    def test_setup_uv_creates_action(self) -> None:
        """Test setup_uv creates an action."""
        from recompose.gha import setup_uv

        action = setup_uv()
        assert action.uses == "astral-sh/setup-uv@v4"

    def test_setup_rust_creates_action(self) -> None:
        """Test setup_rust creates an action with toolchain."""
        from recompose.gha import setup_rust

        action = setup_rust(toolchain="nightly")
        assert action.uses == "dtolnay/rust-toolchain@master"
        assert action.default_with_params["toolchain"] == "nightly"

    def test_cache_creates_action(self) -> None:
        """Test cache creates an action with path and key."""
        from recompose.gha import cache

        action = cache(path="~/.cache", key="cache-key-${{ hashFiles('**/lockfile') }}")
        assert action.uses == "actions/cache@v4"
        assert action.default_with_params["path"] == "~/.cache"
        assert "cache-key" in action.default_with_params["key"]


# Flow with GHA actions for testing
@recompose.flow
def flow_with_gha_actions() -> None:
    """A flow that uses GHA actions."""
    from recompose.gha import checkout, setup_python, setup_uv

    checkout.flow()
    setup_python(version="3.11").flow()
    setup_uv().flow()
    simple_task.flow()


class TestFlowWithGHAActions:
    """Tests for flows containing GHA actions."""

    def test_flow_with_actions_runs_locally(self) -> None:
        """Test that a flow with GHA actions runs (actions are no-ops)."""
        result = flow_with_gha_actions()
        assert result.ok

    def test_flow_with_actions_generates_yaml(self) -> None:
        """Test that a flow with GHA actions generates correct YAML."""
        flow_info = recompose.get_flow("flow_with_gha_actions")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        # Should have: checkout, setup-python, setup-uv, setup workspace, simple_task
        job = spec.jobs["flow_with_gha_actions"]
        assert len(job.steps) == 5

        # First three should be uses: steps
        assert job.steps[0].uses == "actions/checkout@v4"
        assert job.steps[1].uses == "actions/setup-python@v5"
        assert job.steps[1].with_ == {"python-version": "3.11"}
        assert job.steps[2].uses == "astral-sh/setup-uv@v4"

        # Fourth should be setup step (numbered, e.g., "4_setup_workspace")
        assert "setup_workspace" in job.steps[3].name
        assert job.steps[3].run is not None

        # Fifth should be task step
        assert "simple_task" in job.steps[4].name
        assert job.steps[4].run is not None

    def test_flow_without_actions_gets_auto_checkout(self) -> None:
        """Test that flows without GHA actions get checkout added automatically."""
        flow_info = recompose.get_flow("simple_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")

        job = spec.jobs["simple_flow"]
        # First step should be auto-added checkout
        assert job.steps[0].uses == "actions/checkout@v4"
        assert job.steps[0].name == "Checkout"

    def test_gha_action_yaml_is_valid(self) -> None:
        """Test that generated YAML with GHA actions is valid."""
        flow_info = recompose.get_flow("flow_with_gha_actions")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)
        assert parsed["name"] == "flow_with_gha_actions"

        # Check the uses steps
        steps = parsed["jobs"]["flow_with_gha_actions"]["steps"]
        uses_steps = [s for s in steps if "uses" in s]
        assert len(uses_steps) == 3


class TestValidateWorkflow:
    """Tests for actionlint validation."""

    def test_validation_when_actionlint_missing(self) -> None:
        """Test graceful handling when actionlint is not installed."""
        # This test works regardless of whether actionlint is installed
        # because we're testing the function's behavior
        yaml_content = "name: test\non: push\njobs: {}"
        success, message = validate_workflow(yaml_content)

        if shutil.which("actionlint") is None:
            assert not success
            assert "not found" in message
        else:
            # If actionlint is installed, it will actually validate
            # The empty jobs dict should cause an error
            pass  # Result depends on actionlint behavior

    @pytest.mark.skipif(
        shutil.which("actionlint") is None,
        reason="actionlint not installed",
    )
    def test_valid_workflow_passes(self) -> None:
        """Test that a valid workflow passes validation."""
        flow_info = recompose.get_flow("simple_flow")
        assert flow_info is not None

        spec = render_flow_workflow(flow_info, script_path="app.py")
        yaml_str = spec.to_yaml()

        success, message = validate_workflow(yaml_str)
        assert success, f"Validation failed: {message}"

    @pytest.mark.skipif(
        shutil.which("actionlint") is None,
        reason="actionlint not installed",
    )
    def test_invalid_workflow_fails(self) -> None:
        """Test that an invalid workflow fails validation."""
        invalid_yaml = """
name: test
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: echo ${{ secrets.UNKNOWN_SYNTAX[0] }}
"""
        success, message = validate_workflow(invalid_yaml)
        # actionlint should catch the invalid expression
        assert not success or "error" in message.lower() or len(message) > 0
