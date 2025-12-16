"""Tests for flow composition."""

from recompose import Err, Ok, Result, flow, get_flow_registry, task


def test_flow_registers():
    """Test that @flow registers the flow."""

    @task
    def inner_task() -> Result[str]:
        return Ok("done")

    @flow
    def my_test_flow() -> None:
        inner_task()

    registry = get_flow_registry()
    assert any("my_test_flow" in key for key in registry)


def test_flow_returns_result():
    """Test that flows return Result[None]."""

    @task
    def simple_task() -> Result[int]:
        return Ok(42)

    @flow
    def simple_flow() -> None:
        simple_task()

    result = simple_flow()
    assert result.ok
    assert result.value() is None  # Flows always return None


def test_flow_can_call_tasks():
    """Test that flows can call tasks via ()."""

    @task
    def add_one(*, x: int) -> Result[int]:
        return Ok(x + 1)

    @flow
    def incrementing_flow(*, start: int) -> None:
        add_one(x=start)

    result = incrementing_flow(start=10)
    assert result.ok


def test_flow_tracks_task_executions():
    """Test that FlowContext tracks task executions."""

    @task
    def tracked_task_a() -> Result[str]:
        return Ok("a")

    @task
    def tracked_task_b() -> Result[str]:
        return Ok("b")

    @flow
    def tracking_flow() -> None:
        _a = tracked_task_a()
        tracked_task_b()

    result = tracking_flow()
    assert result.ok

    # Check that executions were tracked
    flow_ctx = getattr(result, "_flow_context", None)
    assert flow_ctx is not None
    assert len(flow_ctx.executions) == 2
    assert flow_ctx.executions[0].task_name == "tracked_task_a"
    assert flow_ctx.executions[1].task_name == "tracked_task_b"
    assert flow_ctx.all_succeeded


def test_flow_passes_results_between_tasks():
    """Test passing results from one task to another."""

    @task
    def multiply(*, x: int, y: int) -> Result[int]:
        return Ok(x * y)

    @task
    def add(*, x: int, y: int) -> Result[int]:
        return Ok(x + y)

    @flow
    def math_flow(*, a: int, b: int) -> None:
        mul_result = multiply(x=a, y=b)
        add(x=mul_result.value(), y=10)

    result = math_flow(a=3, b=4)
    assert result.ok


def test_flow_handles_task_failure():
    """Test that flows handle task failures correctly."""

    @task
    def failing_task() -> Result[str]:
        return Err("Task failed")

    @task
    def succeeding_task(*, dep: str) -> Result[str]:
        return Ok("success")

    @flow
    def flow_with_failure() -> None:
        r = failing_task()
        # This won't run because failing_task fails
        succeeding_task(dep=r.value())

    result = flow_with_failure()
    assert result.failed
    assert result.error == "Task failed"


def test_flow_catches_exceptions():
    """Test that flows catch exceptions and convert to Err."""

    @task
    def throwing_task() -> Result[str]:
        raise ValueError("Task exception")

    @flow
    def throwing_flow() -> None:
        throwing_task()

    result = throwing_flow()
    assert result.failed
    assert "ValueError" in result.error
    assert "Task exception" in result.error


def test_flow_with_arguments():
    """Test flows with keyword arguments."""

    @task
    def format_task(*, name: str, count: int) -> Result[str]:
        return Ok(f"{name} x {count}")

    @flow
    def parameterized_flow(*, name: str, count: int = 1) -> None:
        format_task(name=name, count=count)

    result = parameterized_flow(name="test")
    assert result.ok

    result2 = parameterized_flow(name="hello", count=5)
    assert result2.ok


def test_flow_preserves_docstring():
    """Test that flow docstrings are preserved."""

    @task
    def doc_task() -> Result[None]:
        return Ok(None)

    @flow
    def documented_flow() -> None:
        """This is a documented flow."""
        doc_task()

    assert documented_flow.__doc__ == "This is a documented flow."


def test_flow_timing():
    """Test that flow tracks timing."""
    import time

    @task
    def slow_task() -> Result[None]:
        time.sleep(0.01)
        return Ok(None)

    @flow
    def timed_flow() -> None:
        slow_task()

    result = timed_flow()
    assert result.ok

    flow_ctx = getattr(result, "_flow_context", None)
    assert flow_ctx is not None
    assert len(flow_ctx.executions) == 1
    assert flow_ctx.executions[0].duration >= 0.01
    assert flow_ctx.total_duration >= 0.01


def test_flow_auto_fails_on_task_failure():
    """Test that flows automatically stop when a task fails."""
    executed_tasks = []

    @task
    def task_a() -> Result[str]:
        executed_tasks.append("a")
        return Ok("a done")

    @task
    def task_b_fails(*, dep: str) -> Result[str]:
        executed_tasks.append("b")
        return Err("B failed!")

    @task
    def task_c(*, dep: str) -> Result[str]:
        executed_tasks.append("c")
        return Ok("c done")

    @flow
    def auto_fail_flow() -> None:
        a = task_a()
        b = task_b_fails(dep=a.value())  # This fails - should stop here
        task_c(dep=b.value())  # This won't run

    executed_tasks.clear()
    result = auto_fail_flow()

    # Flow should have failed
    assert result.failed
    assert result.error == "B failed!"

    # Only tasks a and b should have run
    assert executed_tasks == ["a", "b"]

    # FlowContext should show the executions
    flow_ctx = getattr(result, "_flow_context", None)
    assert flow_ctx is not None
    assert len(flow_ctx.executions) == 2
    assert flow_ctx.executions[0].task_name == "task_a"
    assert flow_ctx.executions[0].result.ok
    assert flow_ctx.executions[1].task_name == "task_b_fails"
    assert flow_ctx.executions[1].result.failed


def test_flow_requires_tasks():
    """Test that flows must have at least one task."""

    @flow
    def empty_flow() -> None:
        pass  # No tasks

    import pytest

    with pytest.raises(ValueError, match="has no tasks"):
        empty_flow()
