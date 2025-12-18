"""Tests for workspace management and subprocess isolation."""

import json
from pathlib import Path

import pytest

import recompose
from recompose.result import Err, Ok
from recompose.workspace import (
    FlowParams,
    create_workspace,
    read_params,
    read_step_result,
    step_result_exists,
    write_params,
    write_step_result,
)


class TestFlowParams:
    """Tests for FlowParams serialization."""

    def test_to_json_and_back(self) -> None:
        """FlowParams can be serialized and deserialized."""
        params = FlowParams(
            flow_name="test_flow",
            params={"repo": "main", "clean": True},
            steps=["1_fetch", "2_build", "3_test"],
            created_at="2024-01-01T00:00:00",
            module_name="my.module",
        )

        json_str = params.to_json()
        restored = FlowParams.from_json(json_str)

        assert restored.flow_name == params.flow_name
        assert restored.params == params.params
        assert restored.steps == params.steps
        assert restored.created_at == params.created_at
        assert restored.module_name == params.module_name


class TestWorkspaceIO:
    """Tests for workspace read/write operations."""

    def test_create_workspace_with_explicit_path(self, tmp_path: Path) -> None:
        """create_workspace uses explicit path when provided."""
        ws = create_workspace("test_flow", workspace=tmp_path / "my_workspace")
        assert ws == tmp_path / "my_workspace"
        assert ws.exists()

    def test_write_and_read_params(self, tmp_path: Path) -> None:
        """Parameters can be written and read back."""
        ws = tmp_path / "workspace"
        params = FlowParams(
            flow_name="build",
            params={"repo": "test"},
            steps=["1_a", "2_b"],
            created_at="2024-01-01T00:00:00",
            module_name="test.module",
        )

        write_params(ws, params)
        restored = read_params(ws)

        assert restored.flow_name == params.flow_name
        assert restored.params == params.params

    def test_read_params_missing_file(self, tmp_path: Path) -> None:
        """read_params raises when _params.json doesn't exist."""
        with pytest.raises(FileNotFoundError):
            read_params(tmp_path)

    def test_write_and_read_step_result_success(self, tmp_path: Path) -> None:
        """Step results can be written and read back."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        result = Ok("/path/to/output")
        write_step_result(ws, "1_fetch", result)

        restored = read_step_result(ws, "1_fetch")
        assert restored.ok
        assert restored.value() == "/path/to/output"

    def test_write_and_read_step_result_failure(self, tmp_path: Path) -> None:
        """Failed results preserve error and traceback."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        result: recompose.Result[str] = Err("Something went wrong", traceback="Traceback...")
        write_step_result(ws, "2_build", result)

        restored = read_step_result(ws, "2_build")
        assert restored.failed
        assert restored.error == "Something went wrong"
        assert restored.traceback == "Traceback..."

    def test_read_step_result_missing(self, tmp_path: Path) -> None:
        """read_step_result returns Err when file doesn't exist."""
        result = read_step_result(tmp_path, "nonexistent")
        assert result.failed
        assert result.error is not None
        assert "not found" in result.error.lower()

    def test_step_result_exists(self, tmp_path: Path) -> None:
        """step_result_exists correctly checks for result files."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        assert not step_result_exists(ws, "1_fetch")

        write_step_result(ws, "1_fetch", Ok("done"))
        assert step_result_exists(ws, "1_fetch")

    def test_serialize_complex_value(self, tmp_path: Path) -> None:
        """Complex values are serialized with type info and restored properly."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        # Path objects should be serialized with type info
        result = Ok(Path("/some/path"))
        write_step_result(ws, "step", result)

        # Check raw JSON has type wrapper with Path-related type key
        data = json.loads((ws / "step.json").read_text())
        assert "__type__" in data["value"]
        assert "Path" in data["value"]["__type__"]  # Could be pathlib.Path or pathlib._local.Path
        assert data["value"]["__value__"] == "/some/path"

        # Read back should restore the Path type
        restored = read_step_result(ws, "step")
        assert restored.value() == Path("/some/path")
        assert isinstance(restored.value(), Path)


class TestFlowPlanSteps:
    """Tests for FlowPlan step assignment."""

    def test_assign_step_names(self) -> None:
        """assign_step_names creates sequential numbered names."""

        @recompose.task
        def step_a() -> recompose.Result[str]:
            return recompose.Ok("a")

        @recompose.task
        def step_b(*, dep: str) -> recompose.Result[str]:
            return recompose.Ok("b")

        @recompose.flow
        def test_flow() -> None:
            a = step_a()
            step_b(dep=a.value())

        plan = test_flow.plan
        # Note: step names are already assigned at decoration time with eager planning

        steps = plan.get_steps()
        assert len(steps) == 2
        # Step names have "step_" prefix for valid GHA step IDs
        assert steps[0][0] == "step_1_step_a"
        assert steps[1][0] == "step_2_step_b"

    def test_get_step_by_number(self) -> None:
        """Steps can be retrieved by number."""

        @recompose.task
        def task_x() -> recompose.Result[str]:
            return recompose.Ok("x")

        @recompose.flow
        def simple_flow() -> None:
            task_x()

        plan = simple_flow.plan
        # Note: step names are already assigned at decoration time with eager planning

        # Can still retrieve by number
        node = plan.get_step("1")
        assert node is not None
        assert node.task_info.name == "task_x"

    def test_get_step_by_full_name(self) -> None:
        """Steps can be retrieved by full name."""

        @recompose.task
        def my_task() -> recompose.Result[str]:
            return recompose.Ok("result")

        @recompose.flow
        def flow_for_lookup() -> None:
            my_task()

        plan = flow_for_lookup.plan
        # Note: step names are already assigned at decoration time with eager planning

        # Full name now includes "step_" prefix
        node = plan.get_step("step_1_my_task")
        assert node is not None
        assert node.step_name == "step_1_my_task"


# =============================================================================
# Module-level flows for subprocess isolation tests
# These must be at module level so subprocesses can find them when importing
# =============================================================================


@recompose.task
def _isolated_step_one() -> recompose.Result[str]:
    return recompose.Ok("one")


@recompose.task
def _isolated_step_two(*, prev: str) -> recompose.Result[str]:
    return recompose.Ok(f"{prev}-two")


@recompose.task
def _isolated_step_three(*, prev: str) -> recompose.Result[str]:
    return recompose.Ok(f"{prev}-three")


@recompose.flow
def _isolated_simple_pipeline() -> None:
    a = _isolated_step_one()
    b = _isolated_step_two(prev=a.value())
    _isolated_step_three(prev=b.value())


@recompose.task
def _isolated_echo_param(*, value: str) -> recompose.Result[str]:
    return recompose.Ok(f"got: {value}")


@recompose.task
def _isolated_process(*, input: str) -> recompose.Result[str]:
    return recompose.Ok(f"processed: {input}")


@recompose.flow
def _isolated_param_flow(*, name: str = "default") -> None:
    v = _isolated_echo_param(value=name)
    _isolated_process(input=v.value())


# App for isolated flow tests - must be at module level for subprocess isolation
_isolated_app = recompose.App(
    commands=[_isolated_simple_pipeline, _isolated_param_flow],
)


class TestRunIsolated:
    """Tests for subprocess execution of flows."""

    def test_flow_executes_with_subprocess_isolation(self) -> None:
        """Direct flow call executes all steps as subprocesses."""
        import tempfile
        from pathlib import Path

        # Set up the isolated app context for this test
        _isolated_app.setup_context()

        # Uses module-level flow _isolated_simple_pipeline
        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)
            result = _isolated_simple_pipeline(workspace=workspace)
            assert result.ok, f"Flow execution failed: {result.error}"

            # Verify step results were actually written (proves steps ran)
            step1_result = read_step_result(workspace, "step_1__isolated_step_one")
            assert step1_result.ok, f"Step 1 didn't write result: {step1_result.error}"
            assert step1_result.value() == "one"

            step2_result = read_step_result(workspace, "step_2__isolated_step_two")
            assert step2_result.ok, f"Step 2 didn't write result: {step2_result.error}"
            assert step2_result.value() == "one-two"

            step3_result = read_step_result(workspace, "step_3__isolated_step_three")
            assert step3_result.ok, f"Step 3 didn't write result: {step3_result.error}"
            assert step3_result.value() == "one-two-three"

    def test_flow_with_params(self) -> None:
        """Flow parameters are passed correctly to steps."""
        import tempfile
        from pathlib import Path

        # Set up the isolated app context for this test
        _isolated_app.setup_context()

        # Uses module-level flow _isolated_param_flow
        with tempfile.TemporaryDirectory() as tmpdir:
            workspace = Path(tmpdir)
            result = _isolated_param_flow(name="test-value", workspace=workspace)
            assert result.ok, f"Flow execution failed: {result.error}"

            # Verify the parameter was passed correctly
            step1_result = read_step_result(workspace, "step_1__isolated_echo_param")
            assert step1_result.ok, f"Step 1 didn't write result: {step1_result.error}"
            assert step1_result.value() == "got: test-value"

            step2_result = read_step_result(workspace, "step_2__isolated_process")
            assert step2_result.ok, f"Step 2 didn't write result: {step2_result.error}"
            assert step2_result.value() == "processed: got: test-value"
