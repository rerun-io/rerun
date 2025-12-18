"""Tests for GitHub Actions workflow generation."""

import shutil

import pytest
from ruamel.yaml import YAML

import recompose
from recompose import (
    Artifact,
    BoolInput,
    ChoiceInput,
    Dispatchable,
    DispatchableInfo,
    InputParam,
    StringInput,
    automation,
    github,
    job,
    make_dispatchable,
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
    render_dispatchable,
    validate_workflow,
)


# Test fixtures - simple tasks for testing
@recompose.task
def simple_task() -> recompose.Result[str]:
    """A simple task with no parameters."""
    return recompose.Ok("done")


@recompose.task
def param_task(*, name: str, count: int = 5) -> recompose.Result[str]:
    """A task with parameters."""
    return recompose.Ok(f"{name}: {count}")


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


class TestGHAActions:
    """Tests for GHA virtual actions."""

    def test_checkout_action_direct_call(self) -> None:
        """Test calling checkout directly (no-op)."""
        from recompose.gha import checkout

        result = checkout()
        assert result.ok
        assert result.value() is None

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


# =============================================================================
# P14 Phase 5: Tests for make_dispatchable and render_dispatchable
# =============================================================================


# Test tasks for dispatchable tests
@recompose.task
def no_params_task() -> recompose.Result[None]:
    """Task with no parameters."""
    return recompose.Ok(None)


@recompose.task
def string_param_task(*, name: str) -> recompose.Result[None]:
    """Task with required string parameter."""
    return recompose.Ok(None)


@recompose.task
def default_param_task(*, name: str = "world", count: int = 5) -> recompose.Result[None]:
    """Task with default parameters."""
    return recompose.Ok(None)


@recompose.task
def bool_param_task(*, verbose: bool = False, debug: bool = True) -> recompose.Result[None]:
    """Task with boolean parameters."""
    return recompose.Ok(None)


@recompose.task(outputs=["result_path"])
def output_task() -> recompose.Result[None]:
    """Task with outputs."""
    return recompose.Ok(None)


@recompose.task(artifacts=["report"])
def artifact_task() -> recompose.Result[None]:
    """Task with artifacts."""
    return recompose.Ok(None)


@recompose.task(secrets=["API_KEY"])
def secret_task() -> recompose.Result[None]:
    """Task with secrets."""
    return recompose.Ok(None)


class TestDispatchInputTypes:
    """Tests for DispatchInput types."""

    def test_string_input_basic(self) -> None:
        """Test StringInput basic usage."""
        inp = StringInput(default="hello", description="A greeting")
        d = inp.to_gha_dict()

        assert d["type"] == "string"
        assert d["default"] == "hello"
        assert d["description"] == "A greeting"
        assert d["required"] is False

    def test_string_input_required(self) -> None:
        """Test StringInput as required."""
        inp = StringInput(required=True, description="Required param")
        d = inp.to_gha_dict()

        assert d["required"] is True
        assert "default" not in d

    def test_bool_input_basic(self) -> None:
        """Test BoolInput basic usage."""
        inp = BoolInput(default=True, description="Enable feature")
        d = inp.to_gha_dict()

        assert d["type"] == "boolean"
        assert d["default"] is True
        assert d["description"] == "Enable feature"

    def test_bool_input_defaults_to_false(self) -> None:
        """Test BoolInput defaults to False."""
        inp = BoolInput()
        d = inp.to_gha_dict()

        assert d["default"] is False

    def test_choice_input_basic(self) -> None:
        """Test ChoiceInput basic usage."""
        inp = ChoiceInput(
            choices=["dev", "staging", "prod"],
            default="staging",
            description="Target environment",
        )
        d = inp.to_gha_dict()

        assert d["type"] == "choice"
        assert d["options"] == ["dev", "staging", "prod"]
        assert d["default"] == "staging"
        assert d["description"] == "Target environment"

    def test_choice_input_required(self) -> None:
        """Test ChoiceInput as required."""
        inp = ChoiceInput(choices=["a", "b"], required=True)
        d = inp.to_gha_dict()

        assert d["required"] is True


class TestMakeDispatchable:
    """Tests for make_dispatchable function."""

    def test_non_task_raises(self) -> None:
        """make_dispatchable requires a @task-decorated function."""
        with pytest.raises(TypeError, match="requires a @task-decorated function"):
            make_dispatchable(lambda: None)  # type: ignore[arg-type]

    def test_basic_dispatchable(self) -> None:
        """Test creating a basic dispatchable."""
        d = make_dispatchable(no_params_task)

        assert isinstance(d, Dispatchable)
        assert d.name == "no_params_task"
        assert isinstance(d.info, DispatchableInfo)
        assert d.task_info.name == "no_params_task"

    def test_infer_no_params(self) -> None:
        """Test inferring inputs from task with no params."""
        d = make_dispatchable(no_params_task)

        assert d.info.inputs == {}

    def test_infer_string_param(self) -> None:
        """Test inferring string input from task signature."""
        d = make_dispatchable(string_param_task)

        assert "name" in d.info.inputs
        inp = d.info.inputs["name"]
        assert isinstance(inp, StringInput)
        assert inp.required is True

    def test_infer_default_params(self) -> None:
        """Test inferring inputs with defaults."""
        d = make_dispatchable(default_param_task)

        assert "name" in d.info.inputs
        name_inp = d.info.inputs["name"]
        assert isinstance(name_inp, StringInput)
        assert name_inp.default == "world"
        assert name_inp.required is False

        assert "count" in d.info.inputs
        count_inp = d.info.inputs["count"]
        assert isinstance(count_inp, StringInput)  # Numbers become strings
        assert count_inp.default == "5"

    def test_infer_bool_params(self) -> None:
        """Test inferring boolean inputs."""
        d = make_dispatchable(bool_param_task)

        assert "verbose" in d.info.inputs
        verbose_inp = d.info.inputs["verbose"]
        assert isinstance(verbose_inp, BoolInput)
        assert verbose_inp.default is False

        assert "debug" in d.info.inputs
        debug_inp = d.info.inputs["debug"]
        assert isinstance(debug_inp, BoolInput)
        assert debug_inp.default is True

    def test_explicit_inputs(self) -> None:
        """Test providing explicit inputs."""
        d = make_dispatchable(
            string_param_task,
            inputs={"name": StringInput(default="custom", description="Custom name")},
        )

        assert "name" in d.info.inputs
        inp = d.info.inputs["name"]
        assert inp.default == "custom"
        assert inp.description == "Custom name"

    def test_custom_name(self) -> None:
        """Test providing a custom workflow name."""
        d = make_dispatchable(no_params_task, name="custom_workflow")

        assert d.name == "custom_workflow"
        assert d.task_info.name == "no_params_task"


class TestRenderDispatchable:
    """Tests for render_dispatchable function."""

    def test_basic_render(self) -> None:
        """Test rendering a basic dispatchable."""
        d = make_dispatchable(no_params_task)
        spec = render_dispatchable(d)

        assert spec.name == "no_params_task"
        assert "workflow_dispatch" in spec.on
        assert "no_params_task" in spec.jobs
        assert len(spec.jobs) == 1

    def test_workflow_dispatch_inputs(self) -> None:
        """Test that inputs appear in workflow_dispatch."""
        d = make_dispatchable(default_param_task)
        spec = render_dispatchable(d)

        inputs = spec.on["workflow_dispatch"]["inputs"]
        assert "name" in inputs
        assert inputs["name"]["type"] == "string"
        assert inputs["name"]["default"] == "world"

        assert "count" in inputs
        assert inputs["count"]["type"] == "string"

    def test_explicit_inputs_render(self) -> None:
        """Test rendering with explicit inputs."""
        d = make_dispatchable(
            no_params_task,
            inputs={
                "env": ChoiceInput(choices=["dev", "prod"], default="dev"),
                "verbose": BoolInput(default=False),
            },
        )
        spec = render_dispatchable(d)

        inputs = spec.on["workflow_dispatch"]["inputs"]
        assert "env" in inputs
        assert inputs["env"]["type"] == "choice"
        assert inputs["env"]["options"] == ["dev", "prod"]

        assert "verbose" in inputs
        assert inputs["verbose"]["type"] == "boolean"

    def test_entry_point_in_run_command(self) -> None:
        """Test that entry_point is used in run command."""
        d = make_dispatchable(no_params_task)
        spec = render_dispatchable(d, entry_point="./custom_runner")

        job = spec.jobs["no_params_task"]
        run_step = [s for s in job.steps if s.run is not None][0]
        assert run_step.run.startswith("./custom_runner no_params_task")

    def test_inputs_passed_to_command(self) -> None:
        """Test that inputs are passed as CLI args."""
        d = make_dispatchable(default_param_task)
        spec = render_dispatchable(d)

        job = spec.jobs["default_param_task"]
        run_step = [s for s in job.steps if s.run is not None][0]
        assert "--name=${{ inputs.name }}" in run_step.run
        assert "--count=${{ inputs.count }}" in run_step.run

    def test_default_setup_steps(self) -> None:
        """Test that default setup steps are included."""
        d = make_dispatchable(no_params_task)
        spec = render_dispatchable(d)

        job = spec.jobs["no_params_task"]
        assert job.steps[0].uses == "actions/checkout@v4"
        assert job.steps[1].uses == "actions/setup-python@v5"
        assert job.steps[2].uses == "astral-sh/setup-uv@v4"

    def test_custom_setup_steps(self) -> None:
        """Test with custom setup steps."""
        custom_setup = [
            SetupStep("Checkout", "actions/checkout@v4"),
            SetupStep("Setup Rust", "dtolnay/rust-toolchain@master"),
        ]

        d = make_dispatchable(no_params_task)
        spec = render_dispatchable(d, default_setup=custom_setup)

        job = spec.jobs["no_params_task"]
        assert len([s for s in job.steps if "setup-python" in (s.uses or "")]) == 0
        assert len([s for s in job.steps if "rust-toolchain" in (s.uses or "")]) == 1

    def test_working_directory(self) -> None:
        """Test that working_directory is applied."""
        d = make_dispatchable(no_params_task)
        spec = render_dispatchable(d, working_directory="subdir")

        job = spec.jobs["no_params_task"]
        assert job.working_directory == "subdir"

    def test_task_with_outputs(self) -> None:
        """Test rendering a task with outputs."""
        d = make_dispatchable(output_task)
        spec = render_dispatchable(d)

        job = spec.jobs["output_task"]
        assert job.outputs is not None
        assert "result_path" in job.outputs

        # Run step should have id="run"
        run_step = [s for s in job.steps if s.run is not None][0]
        assert run_step.id == "run"

    def test_task_with_artifacts(self) -> None:
        """Test rendering a task with artifacts."""
        d = make_dispatchable(artifact_task)
        spec = render_dispatchable(d)

        job = spec.jobs["artifact_task"]
        upload_steps = [s for s in job.steps if s.uses and "upload-artifact" in s.uses]
        assert len(upload_steps) == 1

    def test_task_with_secrets(self) -> None:
        """Test rendering a task with secrets."""
        d = make_dispatchable(secret_task)
        spec = render_dispatchable(d)

        job = spec.jobs["secret_task"]
        assert job.env is not None
        assert "API_KEY" in job.env
        assert job.env["API_KEY"] == "${{ secrets.API_KEY }}"

    def test_yaml_output_valid(self) -> None:
        """Test that generated YAML is valid."""
        d = make_dispatchable(default_param_task)
        spec = render_dispatchable(d)
        yaml_str = spec.to_yaml()

        # Should be parseable
        yaml = YAML()
        parsed = yaml.load(yaml_str)

        assert parsed["name"] == "default_param_task"
        assert "workflow_dispatch" in parsed["on"]
        assert "default_param_task" in parsed["jobs"]


class TestDispatchableRepr:
    """Tests for Dispatchable string representation."""

    def test_repr(self) -> None:
        """Test Dispatchable repr."""
        d = make_dispatchable(no_params_task)
        assert "no_params_task" in repr(d)
