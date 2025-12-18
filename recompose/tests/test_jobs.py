"""Tests for the job-based automation framework (P14 Phase 2)."""

from pathlib import Path

import pytest

import recompose
from recompose import (
    Artifact,
    ArtifactRef,
    AutomationInfo,
    ConditionExpr,
    InputParam,
    JobOutputRef,
    JobSpec,
    github,
    job,
    on_pull_request,
    on_push,
    on_schedule,
    on_workflow_dispatch,
)

# =============================================================================
# Test Tasks (fixtures)
# =============================================================================


@recompose.task
def simple_task() -> recompose.Result[None]:
    """A simple task with no outputs."""
    return recompose.Ok(None)


@recompose.task(outputs=["wheel_path", "version"])
def build_wheel() -> recompose.Result[None]:
    """Build a wheel and set outputs."""
    recompose.set_output("wheel_path", "/dist/pkg-1.0.0.whl")
    recompose.set_output("version", "1.0.0")
    return recompose.Ok(None)


@recompose.task(artifacts=["wheel"])
def build_with_artifact() -> recompose.Result[None]:
    """Build and save an artifact."""
    return recompose.Ok(None)


@recompose.task(secrets=["PYPI_TOKEN"])
def publish_task() -> recompose.Result[None]:
    """Publish (requires secret)."""
    return recompose.Ok(None)


@recompose.task
def run_wheel_tests(*, wheel_path: str) -> recompose.Result[None]:
    """Test a wheel."""
    return recompose.Ok(None)


@recompose.task
def run_artifact_tests(*, wheel: Artifact) -> recompose.Result[None]:
    """Test using an artifact."""
    return recompose.Ok(None)


# =============================================================================
# Test Automations
# =============================================================================


class TestJobSpecCreation:
    """Tests for JobSpec creation via job()."""

    def test_job_outside_automation_raises(self) -> None:
        """job() must be called inside @automation."""
        with pytest.raises(RuntimeError, match="can only be called inside"):
            job(simple_task)

    def test_job_with_non_task_raises(self) -> None:
        """job() requires a @task-decorated function."""

        @recompose.automation
        def bad_automation() -> None:
            # This should raise because it's not a task
            job(lambda: None)  # type: ignore[arg-type]

        with pytest.raises(TypeError, match="requires a @task-decorated function"):
            bad_automation()

    def test_job_creates_job_spec(self) -> None:
        """job() creates a JobSpec."""

        @recompose.automation
        def my_automation() -> None:
            j = job(simple_task)
            assert isinstance(j, JobSpec)
            assert j.job_id == "simple_task"
            assert j.task_info.name == "simple_task"

        my_automation()

    def test_job_with_custom_id(self) -> None:
        """job() accepts custom job_id."""

        @recompose.automation
        def my_automation() -> None:
            j = job(simple_task, job_id="custom_id")
            assert j.job_id == "custom_id"

        my_automation()

    def test_duplicate_job_id_raises(self) -> None:
        """Duplicate job_id raises error."""

        @recompose.automation
        def bad_automation() -> None:
            job(simple_task)
            job(simple_task)  # Same task = same default ID

        with pytest.raises(ValueError, match="Duplicate job_id"):
            bad_automation()

    def test_job_with_custom_runner(self) -> None:
        """job() accepts custom runs_on."""

        @recompose.automation
        def my_automation() -> None:
            j = job(simple_task, runs_on="macos-latest")
            assert j.runs_on == "macos-latest"

        my_automation()


class TestAutomationDecorator:
    """Tests for @automation decorator."""

    def test_automation_returns_jobs(self) -> None:
        """@automation returns list of jobs when called."""

        @recompose.automation
        def my_automation() -> None:
            job(simple_task)

        jobs = my_automation()
        assert len(jobs) == 1
        assert jobs[0].job_id == "simple_task"

    def test_automation_plan_method(self) -> None:
        """automation.plan() is an alias for calling."""

        @recompose.automation
        def my_automation() -> None:
            job(simple_task)

        jobs = my_automation.plan()
        assert len(jobs) == 1

    def test_automation_has_info(self) -> None:
        """Automation has info attribute."""

        @recompose.automation
        def my_automation() -> None:
            """My automation docstring."""
            job(simple_task)

        assert isinstance(my_automation.info, AutomationInfo)
        assert my_automation.info.name == "my_automation"
        assert my_automation.info.doc == "My automation docstring."

    def test_automation_with_trigger(self) -> None:
        """Automation accepts trigger parameter."""

        @recompose.automation(trigger=on_push(branches=["main"]))
        def ci() -> None:
            job(simple_task)

        assert ci.info.trigger is not None

    def test_automation_multiple_jobs(self) -> None:
        """Automation can have multiple jobs."""

        @recompose.automation
        def ci() -> None:
            job(simple_task, job_id="lint")
            job(simple_task, job_id="test")
            job(simple_task, job_id="build")

        jobs = ci()
        assert len(jobs) == 3
        job_ids = [j.job_id for j in jobs]
        assert job_ids == ["lint", "test", "build"]


class TestJobOutputRef:
    """Tests for job output references."""

    def test_get_valid_output(self) -> None:
        """JobSpec.get() returns JobOutputRef for valid output."""

        @recompose.automation
        def my_automation() -> None:
            build_job = job(build_wheel)
            ref = build_job.get("wheel_path")
            assert isinstance(ref, JobOutputRef)
            assert ref.job_id == "build_wheel"
            assert ref.output_name == "wheel_path"

        my_automation()

    def test_get_invalid_output_raises(self) -> None:
        """JobSpec.get() raises for undeclared output."""

        @recompose.automation
        def bad_automation() -> None:
            build_job = job(build_wheel)
            build_job.get("nonexistent")

        with pytest.raises(ValueError, match="has no output 'nonexistent'"):
            bad_automation()

    def test_output_ref_to_gha_expr(self) -> None:
        """JobOutputRef generates correct GHA expression."""
        ref = JobOutputRef("build_wheel", "wheel_path")
        assert ref.to_gha_expr() == "${{ needs.build_wheel.outputs.wheel_path }}"

    def test_output_creates_dependency(self) -> None:
        """Using output ref creates dependency."""

        @recompose.automation
        def my_automation() -> None:
            build_job = job(build_wheel)
            test_job = job(
                run_wheel_tests,
                inputs={"wheel_path": build_job.get("wheel_path")},
            )
            # Dependency should be inferred
            deps = test_job.get_all_dependencies()
            assert len(deps) == 1
            assert deps[0].job_id == "build_wheel"

        my_automation()


class TestArtifactRef:
    """Tests for artifact references."""

    def test_artifact_valid(self) -> None:
        """JobSpec.artifact() returns ArtifactRef for valid artifact."""

        @recompose.automation
        def my_automation() -> None:
            build_job = job(build_with_artifact)
            ref = build_job.artifact("wheel")
            assert isinstance(ref, ArtifactRef)
            assert ref.job_id == "build_with_artifact"
            assert ref.artifact_name == "wheel"

        my_automation()

    def test_artifact_invalid_raises(self) -> None:
        """JobSpec.artifact() raises for undeclared artifact."""

        @recompose.automation
        def bad_automation() -> None:
            build_job = job(build_with_artifact)
            build_job.artifact("nonexistent")

        with pytest.raises(ValueError, match="has no artifact 'nonexistent'"):
            bad_automation()

    def test_artifact_creates_dependency(self) -> None:
        """Using artifact ref creates dependency."""

        @recompose.automation
        def my_automation() -> None:
            build_job = job(build_with_artifact)
            test_job = job(
                run_artifact_tests,
                inputs={"wheel": build_job.artifact("wheel")},
            )
            deps = test_job.get_all_dependencies()
            assert len(deps) == 1
            assert deps[0].job_id == "build_with_artifact"

        my_automation()


class TestExplicitDependencies:
    """Tests for explicit needs dependencies."""

    def test_explicit_needs(self) -> None:
        """Jobs can have explicit needs."""

        @recompose.automation
        def my_automation() -> None:
            lint_job = job(simple_task, job_id="lint")
            test_job = job(simple_task, job_id="test", needs=[lint_job])
            assert lint_job in test_job.needs

        my_automation()

    def test_combined_explicit_and_inferred(self) -> None:
        """get_all_dependencies combines explicit and inferred."""

        @recompose.automation
        def my_automation() -> None:
            lint_job = job(simple_task, job_id="lint")
            build_job = job(build_wheel)
            test_job = job(
                run_wheel_tests,
                inputs={"wheel_path": build_job.get("wheel_path")},
                needs=[lint_job],
            )
            all_deps = test_job.get_all_dependencies()
            job_ids = [d.job_id for d in all_deps]
            assert "lint" in job_ids
            assert "build_wheel" in job_ids

        my_automation()


class TestConditionExpressions:
    """Tests for condition expressions."""

    def test_input_condition_equality(self) -> None:
        """InputParam == value creates condition."""
        param = InputParam[str](default="prod")
        param._set_name("env")

        cond = param == "prod"
        assert isinstance(cond, ConditionExpr)
        assert cond.to_gha_expr() == "inputs.env == 'prod'"

    def test_input_condition_inequality(self) -> None:
        """InputParam != value creates condition."""
        param = InputParam[str](default="prod")
        param._set_name("env")

        cond = param != "staging"
        assert cond.to_gha_expr() == "inputs.env != 'staging'"

    def test_input_condition_negation(self) -> None:
        """~InputParam creates negated condition."""
        param = InputParam[bool](default=False)
        param._set_name("skip_tests")

        cond = ~param
        assert "!" in cond.to_gha_expr()

    def test_condition_and(self) -> None:
        """Conditions can be ANDed."""
        p1 = InputParam[str](default="prod")
        p1._set_name("env")
        p2 = InputParam[bool](default=False)
        p2._set_name("force")

        cond = (p1 == "prod") & (p2 == True)  # noqa: E712
        expr = cond.to_gha_expr()
        assert "&&" in expr

    def test_condition_or(self) -> None:
        """Conditions can be ORed."""
        p1 = InputParam[str](default="prod")
        p1._set_name("env")

        cond = (p1 == "prod") | (p1 == "staging")
        expr = cond.to_gha_expr()
        assert "||" in expr

    def test_github_context_ref(self) -> None:
        """GitHub context creates conditions."""
        cond = github.ref_name == "main"
        assert cond.to_gha_expr() == "github.ref_name == 'main'"

    def test_github_context_eq_method(self) -> None:
        """GitHub context .eq() method works."""
        cond = github.event_name.eq("push")
        assert cond.to_gha_expr() == "github.event_name == 'push'"

    def test_complex_condition(self) -> None:
        """Complex conditions work."""
        param = InputParam[str](default="prod")
        param._set_name("env")

        cond = (param == "prod") & github.ref_name.eq("main")
        expr = cond.to_gha_expr()
        assert "inputs.env == 'prod'" in expr
        assert "github.ref_name == 'main'" in expr
        assert "&&" in expr

    def test_condition_evaluate(self) -> None:
        """Conditions can be evaluated at runtime."""
        param = InputParam[str](default="prod")
        param._set_name("env")

        cond = param == "prod"
        assert cond.evaluate({"inputs": {"env": "prod"}}) is True
        assert cond.evaluate({"inputs": {"env": "staging"}}) is False


class TestJobConditions:
    """Tests for job conditions."""

    def test_job_with_condition(self) -> None:
        """Jobs can have conditions."""
        param = InputParam[bool](default=False)
        param._set_name("skip_tests")

        @recompose.automation
        def my_automation() -> None:
            test_job = job(simple_task, condition=~param)
            assert test_job.condition is not None

        my_automation()


class TestTriggers:
    """Tests for trigger types."""

    def test_push_trigger(self) -> None:
        """on_push creates PushTrigger."""
        trigger = on_push(branches=["main"])
        d = trigger.to_gha_dict()
        assert "push" in d
        assert d["push"]["branches"] == ["main"]

    def test_pull_request_trigger(self) -> None:
        """on_pull_request creates PullRequestTrigger."""
        trigger = on_pull_request(branches=["main"])
        d = trigger.to_gha_dict()
        assert "pull_request" in d
        assert d["pull_request"]["branches"] == ["main"]

    def test_schedule_trigger(self) -> None:
        """on_schedule creates ScheduleTrigger."""
        trigger = on_schedule(cron="0 0 * * *")
        d = trigger.to_gha_dict()
        assert "schedule" in d
        assert d["schedule"][0]["cron"] == "0 0 * * *"

    def test_workflow_dispatch_trigger(self) -> None:
        """on_workflow_dispatch creates WorkflowDispatchTrigger."""
        trigger = on_workflow_dispatch()
        d = trigger.to_gha_dict()
        assert "workflow_dispatch" in d

    def test_combined_triggers(self) -> None:
        """Triggers can be combined with |."""
        trigger = on_push(branches=["main"]) | on_pull_request()
        d = trigger.to_gha_dict()
        assert "push" in d
        assert "pull_request" in d


class TestInputParam:
    """Tests for InputParam type."""

    def test_input_param_default(self) -> None:
        """InputParam stores default."""
        param = InputParam[str](default="prod")
        assert param._default == "prod"

    def test_input_param_required(self) -> None:
        """InputParam without default is required."""
        param = InputParam[str]()
        assert param._required is True

    def test_input_param_with_default_not_required(self) -> None:
        """InputParam with default is not required."""
        param = InputParam[str](default="prod")
        assert param._required is False

    def test_input_param_choices(self) -> None:
        """InputParam can have choices."""
        param = InputParam[str](default="prod", choices=["prod", "staging", "dev"])
        assert param._choices == ["prod", "staging", "dev"]

    def test_input_param_bool_raises_in_control_flow(self) -> None:
        """InputParam cannot be used in Python if."""
        param = InputParam[bool](default=False)
        with pytest.raises(TypeError, match="cannot be used in Python control flow"):
            if param:
                pass


class TestArtifactType:
    """Tests for Artifact type."""

    def test_artifact_with_path(self) -> None:
        """Artifact stores path."""
        artifact = Artifact(Path("/tmp/wheel.whl"))
        assert artifact.path == Path("/tmp/wheel.whl")

    def test_artifact_fspath(self) -> None:
        """Artifact supports os.fspath."""
        import os

        artifact = Artifact(Path("/tmp/wheel.whl"))
        assert os.fspath(artifact) == "/tmp/wheel.whl"

    def test_artifact_str_path(self) -> None:
        """Artifact accepts string path."""
        artifact = Artifact("/tmp/wheel.whl")
        assert artifact.path == Path("/tmp/wheel.whl")

    def test_artifact_no_path_raises(self) -> None:
        """Artifact.path raises if not set."""
        artifact = Artifact()
        with pytest.raises(RuntimeError, match="path not set"):
            _ = artifact.path


class TestMatrixJobs:
    """Tests for matrix job configuration."""

    def test_job_with_matrix(self) -> None:
        """Jobs can have matrix configuration."""

        @recompose.automation
        def my_automation() -> None:
            test_job = job(
                simple_task,
                matrix={
                    "python": ["3.10", "3.11", "3.12"],
                    "os": ["ubuntu-latest", "macos-latest"],
                },
            )
            assert test_job.matrix is not None
            assert test_job.matrix["python"] == ["3.10", "3.11", "3.12"]

        my_automation()


class TestAutomationContextIsolation:
    """Tests that automation context is properly isolated."""

    def test_contexts_are_isolated(self) -> None:
        """Each automation call has isolated context."""

        @recompose.automation
        def automation1() -> None:
            job(simple_task, job_id="job1")

        @recompose.automation
        def automation2() -> None:
            job(simple_task, job_id="job2")

        jobs1 = automation1()
        jobs2 = automation2()

        # Each should only see its own jobs
        assert len(jobs1) == 1
        assert jobs1[0].job_id == "job1"
        assert len(jobs2) == 1
        assert jobs2[0].job_id == "job2"

    def test_context_cleaned_up_after_error(self) -> None:
        """Context is cleaned up even if automation raises."""

        @recompose.automation
        def bad_automation() -> None:
            job(simple_task)
            raise ValueError("oops")

        with pytest.raises(ValueError):
            bad_automation()

        # Should be able to run another automation
        @recompose.automation
        def good_automation() -> None:
            job(simple_task)

        jobs = good_automation()
        assert len(jobs) == 1
