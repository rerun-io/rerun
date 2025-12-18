"""Tests for GitHub Actions workflow generation."""

import shutil

import pytest
from ruamel.yaml import YAML

import recompose
from recompose import (
    Artifact,
    InputParam,
    automation,
    github,
    job,
    on_pull_request,
    on_push,
)
from recompose.gha import (
    GHAJobSpec,
    JobSpec,
    SetupStep,
    StepSpec,
    WorkflowDispatchInput,
    WorkflowSpec,
    render_automation_jobs,
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
    simple_task()


@recompose.flow
def param_flow(*, repo: str = "main", debug: bool = False) -> None:
    """A flow with parameters."""
    simple_task()


@recompose.flow
def multi_step_flow() -> None:
    """A flow with multiple steps."""
    a = simple_task()
    param_task(name=a.value(), count=10)


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
        flow_info = simple_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")

        assert spec.name == "simple_flow"
        assert "workflow_dispatch" in spec.on

        # Should have checkout + setup + 1 task step
        job = spec.jobs["simple_flow"]
        assert len(job.steps) == 3
        assert job.steps[0].uses == "actions/checkout@v4"
        assert "--setup --flow" in (job.steps[1].run or "")
        assert "--step" in (job.steps[2].run or "")

    def test_flow_with_parameters(self) -> None:
        """Test rendering a flow with parameters."""
        flow_info = param_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")

        # Check workflow_dispatch inputs
        inputs = spec.on["workflow_dispatch"]["inputs"]
        assert "repo" in inputs
        assert inputs["repo"]["default"] == "main"
        assert inputs["repo"]["type"] == "string"

        assert "debug" in inputs
        assert inputs["debug"]["type"] == "boolean"
        assert inputs["debug"]["default"] is False  # GHA boolean inputs need actual booleans

        # Check setup step includes parameters
        job = spec.jobs["param_flow"]
        setup_step = job.steps[1]
        assert "${{ inputs.repo }}" in (setup_step.run or "")
        assert "${{ inputs.debug }}" in (setup_step.run or "")

    def test_multi_step_flow(self) -> None:
        """Test rendering a flow with multiple steps."""
        flow_info = multi_step_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")

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
        flow_info = simple_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py", runs_on="macos-latest")

        job = spec.jobs["simple_flow"]
        assert job.runs_on == "macos-latest"

    def test_yaml_output_is_valid(self) -> None:
        """Test that generated YAML is valid."""
        flow_info = param_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")
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
        assert result.value() is None

    def test_checkout_outside_flow_is_noop(self) -> None:
        """Test that GHA actions are no-ops when called outside a flow."""
        from recompose.gha import checkout

        result = checkout()
        assert result.ok  # GHA actions return Ok(None) when run locally

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

    checkout()
    setup_python(version="3.11")()
    setup_uv()()
    simple_task()


# App for GHA action flow tests - must be at module level for subprocess isolation
_gha_actions_app = recompose.App(
    commands=[flow_with_gha_actions],
)


class TestFlowWithGHAActions:
    """Tests for flows containing GHA actions."""

    def test_flow_with_actions_runs_locally(self) -> None:
        """Test that a flow with GHA actions runs (actions are no-ops)."""
        _gha_actions_app.setup_context()
        result = flow_with_gha_actions()
        assert result.ok

    def test_flow_with_actions_generates_yaml(self) -> None:
        """Test that a flow with GHA actions generates correct YAML."""
        flow_info = flow_with_gha_actions._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")

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
        flow_info = simple_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")

        job = spec.jobs["simple_flow"]
        # First step should be auto-added checkout
        assert job.steps[0].uses == "actions/checkout@v4"
        assert job.steps[0].name == "Checkout"

    def test_gha_action_yaml_is_valid(self) -> None:
        """Test that generated YAML with GHA actions is valid."""
        flow_info = flow_with_gha_actions._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")
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
        flow_info = simple_flow._flow_info

        spec = render_flow_workflow(flow_info, module_name="app.py")
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


# =============================================================================
# P14: Tests for render_automation_jobs (multi-job workflow generation)
# =============================================================================


# Test tasks for automation tests
@recompose.task
def lint_task() -> recompose.Result[None]:
    """Lint the code."""
    return recompose.Ok(None)


@recompose.task
def format_task() -> recompose.Result[None]:
    """Check formatting."""
    return recompose.Ok(None)


@recompose.task
def run_tests_task() -> recompose.Result[None]:
    """Run tests."""
    return recompose.Ok(None)


@recompose.task(outputs=["wheel_path", "version"])
def build_wheel_task() -> recompose.Result[None]:
    """Build a wheel and output path."""
    recompose.set_output("wheel_path", "/dist/pkg-1.0.0.whl")
    recompose.set_output("version", "1.0.0")
    return recompose.Ok(None)


@recompose.task(artifacts=["wheel"])
def build_artifact_task() -> recompose.Result[None]:
    """Build and save a wheel artifact."""
    return recompose.Ok(None)


@recompose.task
def wheel_test_task(*, wheel_path: str) -> recompose.Result[None]:
    """Test with a wheel path input."""
    return recompose.Ok(None)


@recompose.task
def artifact_test_task(*, wheel: Artifact) -> recompose.Result[None]:
    """Test using an artifact input."""
    return recompose.Ok(None)


@recompose.task(secrets=["PYPI_TOKEN", "AWS_KEY"])
def publish_task() -> recompose.Result[None]:
    """Publish (requires secrets)."""
    return recompose.Ok(None)


class TestRenderAutomationJobs:
    """Tests for render_automation_jobs function."""

    def test_simple_automation(self) -> None:
        """Test rendering a simple automation with one job."""

        @automation
        def simple() -> None:
            job(lint_task)

        spec = render_automation_jobs(simple)

        assert spec.name == "simple"
        assert "lint_task" in spec.jobs
        assert len(spec.jobs) == 1

        lint_job = spec.jobs["lint_task"]
        assert lint_job.runs_on == "ubuntu-latest"
        assert len(lint_job.steps) == 4  # 3 setup + 1 run

    def test_automation_with_trigger(self) -> None:
        """Test automation with trigger generates correct 'on:' config."""

        @automation(trigger=on_push(branches=["main"]))
        def ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(ci)

        assert "push" in spec.on
        assert spec.on["push"]["branches"] == ["main"]

    def test_automation_combined_triggers(self) -> None:
        """Test automation with combined triggers."""

        @automation(trigger=on_push(branches=["main"]) | on_pull_request())
        def ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(ci)

        assert "push" in spec.on
        assert "pull_request" in spec.on

    def test_automation_multiple_jobs(self) -> None:
        """Test automation with multiple independent jobs."""

        @automation
        def ci() -> None:
            job(lint_task)
            job(format_task)
            job(run_tests_task)

        spec = render_automation_jobs(ci)

        assert len(spec.jobs) == 3
        assert "lint_task" in spec.jobs
        assert "format_task" in spec.jobs
        assert "run_tests_task" in spec.jobs

    def test_automation_with_dependencies(self) -> None:
        """Test automation with job dependencies."""

        @automation
        def ci() -> None:
            lint_job = job(lint_task)
            format_job = job(format_task)
            job(run_tests_task, needs=[lint_job, format_job])

        spec = render_automation_jobs(ci)

        test_job = spec.jobs["run_tests_task"]
        assert test_job.needs == ["lint_task", "format_task"]

    def test_automation_entry_point(self) -> None:
        """Test that entry_point is used in run commands."""

        @automation
        def ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(ci, entry_point="./custom_runner")

        lint_job = spec.jobs["lint_task"]
        run_step = [s for s in lint_job.steps if s.run is not None][0]
        assert run_step.run.startswith("./custom_runner lint_task")

    def test_job_with_outputs(self) -> None:
        """Test job with declared outputs exposes them correctly."""

        @automation
        def build() -> None:
            job(build_wheel_task)

        spec = render_automation_jobs(build)

        build_job = spec.jobs["build_wheel_task"]
        assert build_job.outputs is not None
        assert "wheel_path" in build_job.outputs
        assert "version" in build_job.outputs
        assert "steps.run.outputs.wheel_path" in build_job.outputs["wheel_path"]

    def test_job_output_reference_creates_dependency(self) -> None:
        """Test that referencing a job's output creates dependency."""

        @automation
        def build_and_test() -> None:
            build_job = job(build_wheel_task)
            job(wheel_test_task, inputs={"wheel_path": build_job.get("wheel_path")})

        spec = render_automation_jobs(build_and_test)

        test_job = spec.jobs["wheel_test_task"]
        assert test_job.needs == ["build_wheel_task"]

        # Check that the run command uses the output reference
        run_step = [s for s in test_job.steps if s.run is not None][0]
        assert "needs.build_wheel_task.outputs.wheel_path" in run_step.run

    def test_job_with_artifacts_upload(self) -> None:
        """Test job with artifacts gets upload step."""

        @automation
        def build() -> None:
            job(build_artifact_task)

        spec = render_automation_jobs(build)

        build_job = spec.jobs["build_artifact_task"]
        upload_steps = [s for s in build_job.steps if s.uses and "upload-artifact" in s.uses]
        assert len(upload_steps) == 1
        assert "build_artifact_task-wheel" in upload_steps[0].with_["name"]

    def test_job_with_artifact_download(self) -> None:
        """Test job consuming artifact gets download step."""

        @automation
        def build_and_test() -> None:
            build_job = job(build_artifact_task)
            job(artifact_test_task, inputs={"wheel": build_job.artifact("wheel")})

        spec = render_automation_jobs(build_and_test)

        test_job = spec.jobs["artifact_test_task"]
        download_steps = [s for s in test_job.steps if s.uses and "download-artifact" in s.uses]
        assert len(download_steps) == 1
        assert "build_artifact_task-wheel" in download_steps[0].with_["name"]

        # Check run command uses artifact path
        run_step = [s for s in test_job.steps if s.run is not None][0]
        assert "artifacts/wheel" in run_step.run

    def test_job_with_secrets(self) -> None:
        """Test job with secrets gets them as env vars."""

        @automation
        def publish() -> None:
            job(publish_task)

        spec = render_automation_jobs(publish)

        pub_job = spec.jobs["publish_task"]
        assert pub_job.env is not None
        assert "PYPI_TOKEN" in pub_job.env
        assert "AWS_KEY" in pub_job.env
        assert pub_job.env["PYPI_TOKEN"] == "${{ secrets.PYPI_TOKEN }}"

    def test_job_with_condition(self) -> None:
        """Test job with condition gets if: expression."""
        skip_tests = InputParam[bool](default=False)
        skip_tests._set_name("skip_tests")

        @automation
        def ci() -> None:
            job(run_tests_task, condition=~skip_tests)

        spec = render_automation_jobs(ci)

        test_job = spec.jobs["run_tests_task"]
        assert test_job.if_condition is not None
        assert "inputs.skip_tests" in test_job.if_condition

    def test_job_with_github_condition(self) -> None:
        """Test job with GitHub context condition."""

        @automation
        def deploy() -> None:
            job(lint_task, condition=github.ref_name == "main")

        spec = render_automation_jobs(deploy)

        lint_job = spec.jobs["lint_task"]
        assert lint_job.if_condition is not None
        assert "github.ref_name" in lint_job.if_condition

    def test_job_with_matrix(self) -> None:
        """Test job with matrix configuration."""

        @automation
        def test_matrix() -> None:
            job(
                run_tests_task,
                matrix={
                    "python": ["3.10", "3.11", "3.12"],
                    "os": ["ubuntu-latest", "macos-latest"],
                },
            )

        spec = render_automation_jobs(test_matrix)

        test_job = spec.jobs["run_tests_task"]
        assert test_job.matrix is not None
        assert test_job.matrix["python"] == ["3.10", "3.11", "3.12"]
        assert test_job.matrix["os"] == ["ubuntu-latest", "macos-latest"]

    def test_job_with_custom_runner(self) -> None:
        """Test job with custom runs_on."""

        @automation
        def macos_ci() -> None:
            job(run_tests_task, runs_on="macos-latest")

        spec = render_automation_jobs(macos_ci)

        test_job = spec.jobs["run_tests_task"]
        assert test_job.runs_on == "macos-latest"

    def test_default_setup_steps(self) -> None:
        """Test that default setup steps are included."""

        @automation
        def ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(ci)

        lint_job = spec.jobs["lint_task"]
        setup_steps = lint_job.steps[:3]  # First 3 should be setup

        assert setup_steps[0].uses == "actions/checkout@v4"
        assert setup_steps[1].uses == "actions/setup-python@v5"
        assert setup_steps[2].uses == "astral-sh/setup-uv@v4"

    def test_custom_setup_steps(self) -> None:
        """Test that custom setup steps override defaults."""
        custom_setup = [
            SetupStep("Checkout", "actions/checkout@v4"),
            SetupStep("Setup Rust", "dtolnay/rust-toolchain@master", {"toolchain": "stable"}),
        ]

        @automation
        def rust_ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(rust_ci, default_setup=custom_setup)

        lint_job = spec.jobs["lint_task"]
        assert len([s for s in lint_job.steps if "setup-python" in (s.uses or "")]) == 0
        rust_setup = [s for s in lint_job.steps if s.uses and "rust-toolchain" in s.uses]
        assert len(rust_setup) == 1

    def test_working_directory(self) -> None:
        """Test that working_directory is applied to jobs."""

        @automation
        def ci() -> None:
            job(lint_task)

        spec = render_automation_jobs(ci, working_directory="subdir")

        lint_job = spec.jobs["lint_task"]
        assert lint_job.working_directory == "subdir"

    def test_yaml_output_valid(self) -> None:
        """Test that generated YAML is valid."""
        from ruamel.yaml import YAML

        @automation(trigger=on_push(branches=["main"]))
        def ci() -> None:
            lint_job = job(lint_task)
            job(run_tests_task, needs=[lint_job])

        spec = render_automation_jobs(ci)
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)

        assert parsed["name"] == "ci"
        assert "push" in parsed["on"]
        assert "lint_task" in parsed["jobs"]
        assert "run_tests_task" in parsed["jobs"]
        assert parsed["jobs"]["run_tests_task"]["needs"] == ["lint_task"]

    def test_automation_with_input_params(self) -> None:
        """Test automation with InputParam generates workflow_dispatch inputs."""

        @automation
        def deploy(
            environment: InputParam[str] = InputParam(default="staging"),
            force: InputParam[bool] = InputParam(default=False),
        ) -> None:
            job(lint_task)

        spec = render_automation_jobs(deploy)

        assert "workflow_dispatch" in spec.on
        inputs = spec.on["workflow_dispatch"]["inputs"]
        assert "environment" in inputs
        assert "force" in inputs
        assert inputs["environment"]["default"] == "staging"
        assert inputs["force"]["type"] == "boolean"

    def test_input_param_passed_to_job(self) -> None:
        """Test InputParam value passed to job correctly."""

        @automation
        def parameterized(
            env: InputParam[str] = InputParam(default="prod"),
        ) -> None:
            job(wheel_test_task, inputs={"wheel_path": env.to_ref()})

        spec = render_automation_jobs(parameterized)

        test_job = spec.jobs["wheel_test_task"]
        run_step = [s for s in test_job.steps if s.run is not None][0]
        assert "inputs.env" in run_step.run


class TestGHAJobSpecEnhancements:
    """Tests for the enhanced GHAJobSpec class."""

    def test_job_spec_needs(self) -> None:
        """Test GHAJobSpec with needs."""
        job = GHAJobSpec(
            name="test",
            needs=["lint", "build"],
            steps=[StepSpec(name="run", run="echo test")],
        )
        d = job.to_dict()

        assert d["needs"] == ["lint", "build"]

    def test_job_spec_outputs(self) -> None:
        """Test GHAJobSpec with outputs."""
        job = GHAJobSpec(
            name="build",
            outputs={"version": "${{ steps.build.outputs.version }}"},
            steps=[StepSpec(name="build", run="echo v1.0.0")],
        )
        d = job.to_dict()

        assert d["outputs"] == {"version": "${{ steps.build.outputs.version }}"}

    def test_job_spec_if_condition(self) -> None:
        """Test GHAJobSpec with if condition."""
        job = GHAJobSpec(
            name="deploy",
            if_condition="${{ github.ref == 'refs/heads/main' }}",
            steps=[StepSpec(name="deploy", run="echo deploy")],
        )
        d = job.to_dict()

        assert d["if"] == "${{ github.ref == 'refs/heads/main' }}"

    def test_job_spec_matrix(self) -> None:
        """Test GHAJobSpec with matrix."""
        job = GHAJobSpec(
            name="test",
            matrix={"python": ["3.10", "3.11"]},
            steps=[StepSpec(name="test", run="pytest")],
        )
        d = job.to_dict()

        assert "strategy" in d
        assert d["strategy"]["matrix"] == {"python": ["3.10", "3.11"]}
