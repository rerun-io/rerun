"""Tests for local automation execution (P15 Phase 2)."""

import pytest

import recompose
from recompose import Artifact, job
from recompose.jobs import ArtifactRef, InputParamRef, JobOutputRef
from recompose.local_executor import (
    AutomationResult,
    JobResult,
    LocalExecutor,
    _build_cli_args,
    _parse_github_output,
    _resolve_input_value,
    topological_sort,
)

# =============================================================================
# Test Tasks (fixtures)
# =============================================================================


@recompose.task
def passing_task() -> recompose.Result[None]:
    """A task that passes."""
    return recompose.Ok(None)


@recompose.task(outputs=["result"])
def task_with_output() -> recompose.Result[None]:
    """A task that sets an output."""
    recompose.set_output("result", "hello")
    return recompose.Ok(None)


@recompose.task
def task_with_input(*, value: str) -> recompose.Result[None]:
    """A task that takes an input."""
    recompose.out(f"Got value: {value}")
    return recompose.Ok(None)


@recompose.task
def failing_task() -> recompose.Result[None]:
    """A task that fails."""
    return recompose.Err("Task failed intentionally")


@recompose.task(artifacts=["wheel"])
def build_task() -> recompose.Result[None]:
    """A task that builds something."""
    return recompose.Ok(None)


@recompose.task
def verify_task(*, wheel: Artifact) -> recompose.Result[None]:
    """A task that tests/verifies something."""
    return recompose.Ok(None)


# =============================================================================
# Test Topological Sort
# =============================================================================


class TestTopologicalSort:
    """Tests for topological sort of jobs."""

    def test_no_dependencies(self) -> None:
        """Jobs with no dependencies can be in any order."""

        @recompose.automation
        def auto() -> None:
            job(passing_task, job_id="a")
            job(passing_task, job_id="b")
            job(passing_task, job_id="c")

        jobs = auto()
        sorted_jobs = topological_sort(jobs)

        # All jobs should be present
        assert len(sorted_jobs) == 3
        job_ids = {j.job_id for j in sorted_jobs}
        assert job_ids == {"a", "b", "c"}

    def test_linear_dependencies(self) -> None:
        """Jobs with linear dependencies are sorted correctly."""

        @recompose.automation
        def auto() -> None:
            a = job(passing_task, job_id="a")
            b = job(passing_task, job_id="b", needs=[a])
            job(passing_task, job_id="c", needs=[b])

        jobs = auto()
        sorted_jobs = topological_sort(jobs)

        job_ids = [j.job_id for j in sorted_jobs]
        # a must come before b, b must come before c
        assert job_ids.index("a") < job_ids.index("b")
        assert job_ids.index("b") < job_ids.index("c")

    def test_diamond_dependencies(self) -> None:
        """Diamond dependencies are handled correctly."""

        @recompose.automation
        def auto() -> None:
            a = job(passing_task, job_id="a")
            b = job(passing_task, job_id="b", needs=[a])
            c = job(passing_task, job_id="c", needs=[a])
            job(passing_task, job_id="d", needs=[b, c])

        jobs = auto()
        sorted_jobs = topological_sort(jobs)

        job_ids = [j.job_id for j in sorted_jobs]
        # a must come before b and c, b and c must come before d
        assert job_ids.index("a") < job_ids.index("b")
        assert job_ids.index("a") < job_ids.index("c")
        assert job_ids.index("b") < job_ids.index("d")
        assert job_ids.index("c") < job_ids.index("d")


class TestResolveInputValue:
    """Tests for input value resolution."""

    def test_literal_value(self) -> None:
        """Literal values are passed through."""
        assert _resolve_input_value("hello", {}, {}) == "hello"
        assert _resolve_input_value(42, {}, {}) == 42
        assert _resolve_input_value(True, {}, {}) is True

    def test_job_output_ref(self) -> None:
        """JobOutputRef resolves to output value."""
        ref = JobOutputRef("build", "result")
        outputs = {"build": {"result": "value123"}}
        assert _resolve_input_value(ref, outputs, {}) == "value123"

    def test_job_output_ref_missing_job(self) -> None:
        """JobOutputRef raises if job not found."""
        ref = JobOutputRef("missing_job", "result")
        with pytest.raises(ValueError, match="not found"):
            _resolve_input_value(ref, {}, {})

    def test_job_output_ref_missing_output(self) -> None:
        """JobOutputRef raises if output not found."""
        ref = JobOutputRef("build", "missing_output")
        outputs = {"build": {"result": "value"}}
        with pytest.raises(ValueError, match="missing_output"):
            _resolve_input_value(ref, outputs, {})

    def test_input_param_ref(self) -> None:
        """InputParamRef resolves to param value."""
        ref = InputParamRef("my_param")
        params = {"my_param": "param_value"}
        assert _resolve_input_value(ref, {}, params) == "param_value"

    def test_input_param_ref_missing(self) -> None:
        """InputParamRef raises if param not found."""
        ref = InputParamRef("missing_param")
        with pytest.raises(ValueError, match="missing_param"):
            _resolve_input_value(ref, {}, {})

    def test_artifact_ref(self) -> None:
        """ArtifactRef returns placeholder (artifacts handled separately)."""
        ref = ArtifactRef("build", "wheel")
        result = _resolve_input_value(ref, {}, {})
        assert "artifact:" in result


class TestBuildCliArgs:
    """Tests for CLI argument building."""

    def test_empty_inputs(self) -> None:
        """No inputs produces no args."""
        args = _build_cli_args({}, {}, {})
        assert args == []

    def test_string_input(self) -> None:
        """String inputs become --name=value."""
        args = _build_cli_args({"name": "value"}, {}, {})
        assert args == ["--name=value"]

    def test_bool_true_input(self) -> None:
        """True bool becomes --name."""
        args = _build_cli_args({"verbose": True}, {}, {})
        assert args == ["--verbose"]

    def test_bool_false_input(self) -> None:
        """False bool becomes --no-name."""
        args = _build_cli_args({"verbose": False}, {}, {})
        assert args == ["--no-verbose"]

    def test_underscore_conversion(self) -> None:
        """Underscores in names become hyphens."""
        args = _build_cli_args({"my_param": "value"}, {}, {})
        assert args == ["--my-param=value"]

    def test_with_job_output_ref(self) -> None:
        """JobOutputRef is resolved before building args."""
        ref = JobOutputRef("build", "path")
        outputs = {"build": {"path": "/dist/pkg.whl"}}
        args = _build_cli_args({"wheel_path": ref}, outputs, {})
        assert args == ["--wheel-path=/dist/pkg.whl"]


class TestParseGithubOutput:
    """Tests for GITHUB_OUTPUT parsing."""

    def test_simple_key_value(self, tmp_path):
        """Simple key=value lines are parsed."""
        output_file = tmp_path / "output.txt"
        output_file.write_text("key1=value1\nkey2=value2\n")

        result = _parse_github_output(output_file)
        assert result == {"key1": "value1", "key2": "value2"}

    def test_multiline_value(self, tmp_path):
        """Multiline values with delimiter syntax are parsed."""
        output_file = tmp_path / "output.txt"
        output_file.write_text("key<<EOF\nline1\nline2\nEOF\n")

        result = _parse_github_output(output_file)
        assert result == {"key": "line1\nline2"}

    def test_mixed_formats(self, tmp_path):
        """Mix of simple and multiline values."""
        output_file = tmp_path / "output.txt"
        output_file.write_text("simple=value\nmulti<<EOF\nline1\nline2\nEOF\n")

        result = _parse_github_output(output_file)
        assert result == {"simple": "value", "multi": "line1\nline2"}

    def test_missing_file(self, tmp_path):
        """Missing file returns empty dict."""
        output_file = tmp_path / "nonexistent.txt"
        result = _parse_github_output(output_file)
        assert result == {}


class TestJobResult:
    """Tests for JobResult dataclass."""

    def test_success_result(self) -> None:
        """Successful job result."""
        result = JobResult(
            job_id="build",
            success=True,
            elapsed_seconds=1.5,
            outputs={"path": "/dist/pkg.whl"},
        )
        assert result.success
        assert result.outputs["path"] == "/dist/pkg.whl"

    def test_failure_result(self) -> None:
        """Failed job result."""
        result = JobResult(
            job_id="test",
            success=False,
            elapsed_seconds=0.5,
            error="Tests failed",
        )
        assert not result.success
        assert result.error == "Tests failed"


class TestAutomationResult:
    """Tests for AutomationResult dataclass."""

    def test_all_jobs_pass(self) -> None:
        """Automation success when all jobs pass."""
        result = AutomationResult(
            automation_name="ci",
            success=True,
            elapsed_seconds=5.0,
            job_results=[
                JobResult(job_id="lint", success=True, elapsed_seconds=1.0),
                JobResult(job_id="test", success=True, elapsed_seconds=2.0),
            ],
        )
        assert result.success
        assert len(result.failed_jobs) == 0

    def test_some_jobs_fail(self) -> None:
        """Automation failure when some jobs fail."""
        result = AutomationResult(
            automation_name="ci",
            success=False,
            elapsed_seconds=5.0,
            job_results=[
                JobResult(job_id="lint", success=True, elapsed_seconds=1.0),
                JobResult(job_id="test", success=False, elapsed_seconds=2.0, error="Failed"),
            ],
        )
        assert not result.success
        assert len(result.failed_jobs) == 1
        assert result.failed_jobs[0].job_id == "test"


class TestLocalExecutorDryRun:
    """Tests for LocalExecutor dry run mode."""

    def test_dry_run_no_execution(self) -> None:
        """Dry run shows what would run without executing."""

        @recompose.automation
        def auto() -> None:
            job(passing_task, job_id="a")
            job(passing_task, job_id="b")

        executor = LocalExecutor(dry_run=True)
        result = executor.execute(auto)

        # All jobs should "succeed" in dry run
        assert result.success
        assert len(result.job_results) == 2


# Note: Integration tests that actually run subprocesses would require
# setting up a proper test environment with the CLI available.
# Those tests are better done manually or in CI with the full app.
