"""Tests for P14 task enhancements: outputs, artifacts, secrets, and step decorator."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest

import recompose
from recompose import Ok, Result, get_secret, save_artifact, set_output, step, step_decorator, task


class TestTaskOutputs:
    """Tests for task output functionality."""

    def test_task_with_outputs_declaration(self) -> None:
        """Task can declare outputs in decorator."""

        @task(outputs=["version", "path"])
        def build_wheel() -> Result[None]:
            set_output("version", "1.0.0")
            set_output("path", "/dist/pkg.whl")
            return Ok(None)

        assert build_wheel._task_info.outputs == ["version", "path"]

    def test_set_output_stores_value(self) -> None:
        """set_output stores value in result."""

        @task(outputs=["version"])
        def build() -> Result[None]:
            set_output("version", "1.0.0")
            return Ok(None)

        result = build()
        assert result.ok
        assert result.outputs == {"version": "1.0.0"}

    def test_set_output_validates_declaration(self) -> None:
        """set_output fails for undeclared outputs."""

        @task(outputs=["version"])
        def build() -> Result[None]:
            set_output("undeclared", "value")
            return Ok(None)

        result = build()
        assert result.failed
        assert "undeclared" in str(result.error)
        assert "not declared" in str(result.error)

    def test_set_output_outside_task_raises(self) -> None:
        """set_output raises when called outside a task."""
        with pytest.raises(RuntimeError, match="must be called from within a task"):
            set_output("test", "value")

    def test_multiple_outputs(self) -> None:
        """Task can set multiple outputs."""

        @task(outputs=["a", "b", "c"])
        def multi_output() -> Result[None]:
            set_output("a", "1")
            set_output("b", "2")
            set_output("c", "3")
            return Ok(None)

        result = multi_output()
        assert result.ok
        assert result.outputs == {"a": "1", "b": "2", "c": "3"}

    def test_task_without_outputs_declaration(self) -> None:
        """Task without outputs declaration has empty list."""

        @task
        def simple() -> Result[None]:
            return Ok(None)

        assert simple._task_info.outputs == []


class TestTaskArtifacts:
    """Tests for task artifact functionality."""

    def test_task_with_artifacts_declaration(self) -> None:
        """Task can declare artifacts in decorator."""

        @task(artifacts=["wheel", "docs"])
        def build() -> Result[None]:
            return Ok(None)

        assert build._task_info.artifacts == ["wheel", "docs"]

    def test_save_artifact_stores_info(self) -> None:
        """save_artifact stores artifact info in result."""
        with tempfile.NamedTemporaryFile(delete=False, suffix=".whl") as f:
            f.write(b"wheel content")
            wheel_path = Path(f.name)

        try:

            @task(artifacts=["wheel"])
            def build() -> Result[None]:
                save_artifact("wheel", wheel_path)
                return Ok(None)

            result = build()
            assert result.ok
            assert "wheel" in result.artifacts
            assert result.artifacts["wheel"].path == wheel_path
        finally:
            wheel_path.unlink()

    def test_save_artifact_validates_declaration(self) -> None:
        """save_artifact fails for undeclared artifacts."""
        with tempfile.NamedTemporaryFile(delete=False) as f:
            temp_path = Path(f.name)

        try:

            @task(artifacts=["wheel"])
            def build() -> Result[None]:
                save_artifact("undeclared", temp_path)
                return Ok(None)

            result = build()
            assert result.failed
            assert "undeclared" in str(result.error)
        finally:
            temp_path.unlink()

    def test_save_artifact_validates_path_exists(self) -> None:
        """save_artifact fails if path doesn't exist."""

        @task(artifacts=["wheel"])
        def build() -> Result[None]:
            save_artifact("wheel", Path("/nonexistent/path"))
            return Ok(None)

        result = build()
        assert result.failed
        assert "does not exist" in str(result.error)

    def test_save_artifact_outside_task_raises(self) -> None:
        """save_artifact raises when called outside a task."""
        with pytest.raises(RuntimeError, match="must be called from within a task"):
            save_artifact("test", Path("."))


class TestTaskSecrets:
    """Tests for task secrets functionality."""

    def test_task_with_secrets_declaration(self) -> None:
        """Task can declare secrets in decorator."""

        @task(secrets=["API_KEY", "TOKEN"])
        def deploy() -> Result[None]:
            return Ok(None)

        assert deploy._task_info.secrets == ["API_KEY", "TOKEN"]

    def test_get_secret_from_env(self) -> None:
        """get_secret reads from environment variable."""
        os.environ["TEST_SECRET"] = "secret_value"
        try:

            @task(secrets=["TEST_SECRET"])
            def use_secret() -> Result[str]:
                return Ok(get_secret("TEST_SECRET"))

            result = use_secret()
            assert result.ok
            assert result.value() == "secret_value"
        finally:
            del os.environ["TEST_SECRET"]

    def test_get_secret_validates_declaration(self) -> None:
        """get_secret fails for undeclared secrets."""

        @task(secrets=["API_KEY"])
        def use_secret() -> Result[None]:
            get_secret("UNDECLARED")
            return Ok(None)

        result = use_secret()
        assert result.failed
        assert "UNDECLARED" in str(result.error)
        assert "not declared" in str(result.error)

    def test_get_secret_outside_task_raises(self) -> None:
        """get_secret raises when called outside a task."""
        with pytest.raises(RuntimeError, match="must be called from within a task"):
            get_secret("TEST")

    def test_get_secret_not_found_error(self) -> None:
        """get_secret raises if secret not in env or secrets file."""
        # Ensure the secret doesn't exist in env
        env_key = "DEFINITELY_NOT_SET_12345"
        if env_key in os.environ:
            del os.environ[env_key]

        @task(secrets=[env_key])
        def use_secret() -> Result[None]:
            get_secret(env_key)
            return Ok(None)

        result = use_secret()
        assert result.failed
        assert "not found" in str(result.error)


class TestTaskSetup:
    """Tests for task setup functionality."""

    def test_task_with_setup_declaration(self) -> None:
        """Task can declare setup steps in decorator."""
        setup_steps = ["checkout", "setup-python"]

        @task(setup=setup_steps)
        def build() -> Result[None]:
            return Ok(None)

        assert build._task_info.setup == setup_steps

    def test_task_without_setup_is_none(self) -> None:
        """Task without setup declaration has None."""

        @task
        def simple() -> Result[None]:
            return Ok(None)

        assert simple._task_info.setup is None


class TestStepContextManager:
    """Tests for step context manager."""

    def test_step_basic(self, capsys: pytest.CaptureFixture[str]) -> None:
        """step() context manager groups output."""

        @task
        def build() -> Result[None]:
            with step("Compile"):
                print("compiling...")
            return Ok(None)

        result = build()
        assert result.ok

        captured = capsys.readouterr()
        assert "[Compile]" in captured.out
        assert "compiling..." in captured.out

    def test_step_nested(self, capsys: pytest.CaptureFixture[str]) -> None:
        """step() can be nested."""

        @task
        def build() -> Result[None]:
            with step("Build"):
                print("building...")
                with step("Compile"):
                    print("compiling...")
            return Ok(None)

        result = build()
        assert result.ok

        captured = capsys.readouterr()
        assert "[Build]" in captured.out
        assert "[Compile]" in captured.out


class TestStepDecorator:
    """Tests for step decorator."""

    def test_step_decorator_basic(self, capsys: pytest.CaptureFixture[str]) -> None:
        """step_decorator wraps function output."""

        @step_decorator
        def compile_code() -> None:
            print("compiling...")

        @task
        def build() -> Result[None]:
            compile_code()
            return Ok(None)

        result = build()
        assert result.ok

        captured = capsys.readouterr()
        assert "[compile_code]" in captured.out

    def test_step_decorator_with_name(self, capsys: pytest.CaptureFixture[str]) -> None:
        """step_decorator accepts custom name."""

        @step_decorator("Custom Step Name")
        def compile_code() -> None:
            print("compiling...")

        @task
        def build() -> Result[None]:
            compile_code()
            return Ok(None)

        result = build()
        assert result.ok

        captured = capsys.readouterr()
        assert "[Custom Step Name]" in captured.out


class TestTaskDecoratorCombinations:
    """Tests for combining multiple task decorator parameters."""

    def test_all_parameters(self) -> None:
        """Task can have all parameters together."""

        @task(
            outputs=["version"],
            artifacts=["wheel"],
            secrets=["TOKEN"],
            setup=["checkout"],
        )
        def complex_task() -> Result[None]:
            return Ok(None)

        info = complex_task._task_info
        assert info.outputs == ["version"]
        assert info.artifacts == ["wheel"]
        assert info.secrets == ["TOKEN"]
        assert info.setup == ["checkout"]

    def test_outputs_and_value(self) -> None:
        """Task can have both outputs and return value."""

        @task(outputs=["meta"])
        def build() -> Result[str]:
            set_output("meta", "extra info")
            return Ok("build result")

        result = build()
        assert result.ok
        assert result.value() == "build result"
        assert result.outputs == {"meta": "extra info"}
