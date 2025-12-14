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
            script_path="/path/to/script.py",
        )

        json_str = params.to_json()
        restored = FlowParams.from_json(json_str)

        assert restored.flow_name == params.flow_name
        assert restored.params == params.params
        assert restored.steps == params.steps
        assert restored.created_at == params.created_at
        assert restored.script_path == params.script_path


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
            script_path="script.py",
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
        assert restored.value == "/path/to/output"

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
        assert "not found" in result.error.lower()

    def test_step_result_exists(self, tmp_path: Path) -> None:
        """step_result_exists correctly checks for result files."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        assert not step_result_exists(ws, "1_fetch")

        write_step_result(ws, "1_fetch", Ok("done"))
        assert step_result_exists(ws, "1_fetch")

    def test_serialize_complex_value(self, tmp_path: Path) -> None:
        """Complex values are serialized properly."""
        ws = tmp_path / "workspace"
        ws.mkdir()

        # Path objects should be converted to strings
        result = Ok(Path("/some/path"))
        write_step_result(ws, "step", result)

        # Check raw JSON
        data = json.loads((ws / "step.json").read_text())
        assert data["value"] == "/some/path"

        # Read back
        restored = read_step_result(ws, "step")
        assert restored.value == "/some/path"


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
            a = step_a.flow()
            step_b.flow(dep=a)

        plan = test_flow.plan()
        plan.assign_step_names()

        steps = plan.get_steps()
        assert len(steps) == 2
        assert steps[0][0] == "1_step_a"
        assert steps[1][0] == "2_step_b"

    def test_get_step_by_number(self) -> None:
        """Steps can be retrieved by number."""

        @recompose.task
        def task_x() -> recompose.Result[str]:
            return recompose.Ok("x")

        @recompose.flow
        def simple_flow() -> None:
            task_x.flow()

        plan = simple_flow.plan()
        plan.assign_step_names()

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
            my_task.flow()

        plan = flow_for_lookup.plan()
        plan.assign_step_names()

        node = plan.get_step("1_my_task")
        assert node is not None
        assert node.step_name == "1_my_task"


class TestRunIsolated:
    """Tests for run_isolated subprocess execution."""

    def test_run_isolated_basic(self) -> None:
        """run_isolated executes all steps as subprocesses."""
        # Import the flow_demo module to get a real flow
        import sys

        sys.path.insert(0, str(Path(__file__).parent.parent / "examples"))

        from flow_demo import build_pipeline

        result = build_pipeline.run_isolated(repo="test-isolated")

        assert result.ok, f"run_isolated failed: {result.error}"

    def test_run_isolated_creates_workspace_files(self, tmp_path: Path) -> None:
        """run_isolated creates workspace with result files."""
        import os

        # Set environment to use our temp directory
        old_env = os.environ.get("RECOMPOSE_WORKSPACE")
        os.environ["RECOMPOSE_WORKSPACE"] = str(tmp_path)

        try:
            import sys

            sys.path.insert(0, str(Path(__file__).parent.parent / "examples"))

            from flow_demo import build_pipeline

            result = build_pipeline.run_isolated(repo="workspace-test")
            assert result.ok

            # Check that workspace was created
            workspaces = list(tmp_path.glob("build_pipeline_*"))
            assert len(workspaces) >= 1

            ws = workspaces[-1]  # Most recent
            assert (ws / "_params.json").exists()

            # Check params content
            params = read_params(ws)
            assert params.flow_name == "build_pipeline"
            assert params.params["repo"] == "workspace-test"
            assert len(params.steps) == 5

            # Check step results exist
            for step_name in params.steps:
                assert step_result_exists(ws, step_name), f"Missing result for {step_name}"

        finally:
            if old_env is None:
                os.environ.pop("RECOMPOSE_WORKSPACE", None)
            else:
                os.environ["RECOMPOSE_WORKSPACE"] = old_env
