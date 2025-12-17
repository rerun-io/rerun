"""Tests for flow composition."""

import subprocess
import sys
from pathlib import Path

import pytest

from . import flow_test_app

# Path to the test app for subprocess invocation
TEST_APP = Path(__file__).parent / "flow_test_app.py"


def test_flow_has_flow_info():
    """Test that @flow attaches _flow_info to the wrapper."""
    assert hasattr(flow_test_app.simple_flow, "_flow_info")
    assert flow_test_app.simple_flow._flow_info.name == "simple_flow"


def test_flow_returns_result():
    """Test that flows return Result[None]."""
    result = flow_test_app.simple_flow()
    assert result.ok
    assert result.value() is None  # Flows always return None


def test_flow_can_call_tasks():
    """Test that flows can call tasks."""
    result = flow_test_app.arg_flow(initial=10)
    assert result.ok


def test_flow_passes_results_between_tasks():
    """Test passing results from one task to another."""
    result = flow_test_app.math_flow(a=3, b=4)
    assert result.ok


def test_flow_handles_task_failure():
    """Test that flows handle task failures correctly."""
    result = flow_test_app.failure_flow()
    assert result.failed
    assert "failed!" in (result.error or "")


def test_flow_catches_exceptions():
    """Test that flows catch exceptions and convert to Err."""
    result = flow_test_app.throwing_flow()
    assert result.failed
    assert "ValueError" in (result.error or "")
    assert "Task exception" in (result.error or "")


def test_flow_with_arguments():
    """Test flows with keyword arguments."""
    result = flow_test_app.parameterized_flow(name="test")
    assert result.ok

    result2 = flow_test_app.parameterized_flow(name="hello", count=5)
    assert result2.ok


def test_flow_preserves_docstring():
    """Test that flow docstrings are preserved."""
    assert "simple two-step" in (flow_test_app.simple_flow.__doc__ or "")


def test_flow_requires_tasks():
    """Test that flows must have at least one task.

    With eager planning, this error is raised at decoration time, not call time.
    """
    from recompose import flow

    with pytest.raises(ValueError, match="has no tasks"):

        @flow
        def empty_flow() -> None:
            pass  # No tasks


def test_flow_fail_fast():
    """Test that flows stop on first failure."""
    result = flow_test_app.fail_fast_flow()
    assert result.failed
    assert "failed!" in (result.error or "")


def test_flow_cli_invocation():
    """Test that flows can be invoked via CLI."""
    # Use kebab-case command name
    result = subprocess.run(
        [sys.executable, str(TEST_APP), "simple-flow"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"CLI failed: {result.stderr}"


def test_flow_cli_with_args():
    """Test CLI invocation with arguments."""
    # Use kebab-case command name
    result = subprocess.run(
        [sys.executable, str(TEST_APP), "arg-flow", "--initial", "42"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"CLI failed: {result.stderr}"


def test_flow_cli_failure():
    """Test that CLI exits with error on flow failure."""
    # Use kebab-case command name
    result = subprocess.run(
        [sys.executable, str(TEST_APP), "fail-fast-flow"],
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0, "Expected non-zero exit code for failing flow"
