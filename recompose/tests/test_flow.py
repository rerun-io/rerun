"""Tests for flow composition."""

import recompose
from recompose import Err, Ok, Result, flow, get_flow_context, get_flow_registry, task


def test_flow_registers():
    """Test that @flow registers the flow."""

    @flow
    def my_test_flow() -> Result[str]:
        return Ok("done")

    registry = get_flow_registry()
    assert any("my_test_flow" in key for key in registry)


def test_flow_returns_result():
    """Test that flows return Result."""

    @flow
    def simple_flow() -> Result[int]:
        return Ok(42)

    result = simple_flow()
    assert result.ok
    assert result.value == 42


def test_flow_can_call_tasks():
    """Test that flows can call tasks."""

    @task
    def add_one(*, x: int) -> Result[int]:
        return Ok(x + 1)

    @flow
    def incrementing_flow(*, start: int) -> Result[int]:
        r = add_one(x=start)
        return r

    result = incrementing_flow(start=10)
    assert result.ok
    assert result.value == 11


def test_flow_tracks_task_executions():
    """Test that FlowContext tracks task executions."""

    @task
    def tracked_task_a() -> Result[str]:
        return Ok("a")

    @task
    def tracked_task_b() -> Result[str]:
        return Ok("b")

    flow_ctx_captured = None

    @flow
    def tracking_flow() -> Result[str]:
        nonlocal flow_ctx_captured
        tracked_task_a()
        tracked_task_b()
        flow_ctx_captured = get_flow_context()
        return Ok("done")

    result = tracking_flow()
    assert result.ok

    # Check that executions were tracked
    assert flow_ctx_captured is not None
    assert len(flow_ctx_captured.executions) == 2
    assert flow_ctx_captured.executions[0].task_name == "tracked_task_a"
    assert flow_ctx_captured.executions[1].task_name == "tracked_task_b"
    assert flow_ctx_captured.all_succeeded


def test_flow_passes_results_between_tasks():
    """Test passing results from one task to another."""

    @task
    def multiply(*, x: int, y: int) -> Result[int]:
        return Ok(x * y)

    @task
    def add(*, x: int, y: int) -> Result[int]:
        return Ok(x + y)

    @flow
    def math_flow(*, a: int, b: int) -> Result[int]:
        mul_result = multiply(x=a, y=b)
        if mul_result.failed:
            return mul_result
        add_result = add(x=mul_result.value, y=10)
        return add_result

    result = math_flow(a=3, b=4)
    assert result.ok
    assert result.value == 22  # (3 * 4) + 10 = 22


def test_flow_handles_task_failure():
    """Test that flows handle task failures correctly."""

    @task
    def failing_task() -> Result[str]:
        return Err("Task failed")

    @task
    def succeeding_task() -> Result[str]:
        return Ok("success")

    @flow
    def flow_with_failure() -> Result[str]:
        r = failing_task()
        if r.failed:
            return r
        # This should not execute
        return succeeding_task()

    result = flow_with_failure()
    assert result.failed
    assert result.error == "Task failed"


def test_flow_catches_exceptions():
    """Test that flows catch exceptions and convert to Err."""

    @flow
    def throwing_flow() -> Result[str]:
        raise ValueError("Flow exception")

    result = throwing_flow()
    assert result.failed
    assert "ValueError" in result.error
    assert "Flow exception" in result.error


def test_flow_with_arguments():
    """Test flows with keyword arguments."""

    @flow
    def parameterized_flow(*, name: str, count: int = 1) -> Result[str]:
        return Ok(f"{name} x {count}")

    result = parameterized_flow(name="test")
    assert result.ok
    assert result.value == "test x 1"

    result2 = parameterized_flow(name="hello", count=5)
    assert result2.ok
    assert result2.value == "hello x 5"


def test_flow_preserves_docstring():
    """Test that flow docstrings are preserved."""

    @flow
    def documented_flow() -> Result[None]:
        """This is a documented flow."""
        return Ok(None)

    assert documented_flow.__doc__ == "This is a documented flow."


def test_flow_timing():
    """Test that flow tracks timing."""
    import time

    @task
    def slow_task() -> Result[None]:
        time.sleep(0.01)
        return Ok(None)

    flow_ctx_captured = None

    @flow
    def timed_flow() -> Result[None]:
        nonlocal flow_ctx_captured
        slow_task()
        flow_ctx_captured = get_flow_context()
        return Ok(None)

    timed_flow()

    assert flow_ctx_captured is not None
    assert len(flow_ctx_captured.executions) == 1
    assert flow_ctx_captured.executions[0].duration >= 0.01
    assert flow_ctx_captured.total_duration >= 0.01
