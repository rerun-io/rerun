"""Tests for flow composition."""

import recompose
from recompose import Err, Ok, Result, flow, get_flow_registry, task


def test_flow_registers():
    """Test that @flow registers the flow."""

    @flow
    def my_test_flow():
        @task
        def inner_task() -> Result[str]:
            return Ok("done")

        return inner_task.flow()

    registry = get_flow_registry()
    assert any("my_test_flow" in key for key in registry)


def test_flow_returns_result():
    """Test that flows return Result."""

    @task
    def simple_task() -> Result[int]:
        return Ok(42)

    @flow
    def simple_flow():
        return simple_task.flow()

    result = simple_flow()
    assert result.ok
    assert result.value == 42


def test_flow_can_call_tasks():
    """Test that flows can call tasks via .flow()."""

    @task
    def add_one(*, x: int) -> Result[int]:
        return Ok(x + 1)

    @flow
    def incrementing_flow(*, start: int):
        return add_one.flow(x=start)

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

    @flow
    def tracking_flow():
        a = tracked_task_a.flow()
        b = tracked_task_b.flow()
        return b

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
    def math_flow(*, a: int, b: int):
        mul_result = multiply.flow(x=a, y=b)
        add_result = add.flow(x=mul_result, y=10)
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
    def succeeding_task(*, dep: str) -> Result[str]:
        return Ok("success")

    @flow
    def flow_with_failure():
        r = failing_task.flow()
        # This won't run because failing_task fails
        return succeeding_task.flow(dep=r)

    result = flow_with_failure()
    assert result.failed
    assert result.error == "Task failed"


def test_flow_catches_exceptions():
    """Test that flows catch exceptions and convert to Err."""

    @task
    def throwing_task() -> Result[str]:
        raise ValueError("Task exception")

    @flow
    def throwing_flow():
        return throwing_task.flow()

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
    def parameterized_flow(*, name: str, count: int = 1):
        return format_task.flow(name=name, count=count)

    result = parameterized_flow(name="test")
    assert result.ok
    assert result.value == "test x 1"

    result2 = parameterized_flow(name="hello", count=5)
    assert result2.ok
    assert result2.value == "hello x 5"


def test_flow_preserves_docstring():
    """Test that flow docstrings are preserved."""

    @task
    def doc_task() -> Result[None]:
        return Ok(None)

    @flow
    def documented_flow():
        """This is a documented flow."""
        return doc_task.flow()

    assert documented_flow.__doc__ == "This is a documented flow."


def test_flow_timing():
    """Test that flow tracks timing."""
    import time

    @task
    def slow_task() -> Result[None]:
        time.sleep(0.01)
        return Ok(None)

    @flow
    def timed_flow():
        return slow_task.flow()

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
    def auto_fail_flow():
        a = task_a.flow()
        b = task_b_fails.flow(dep=a)  # This fails - should stop here
        c = task_c.flow(dep=b)  # This won't run
        return c

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


def test_flow_must_return_task_node():
    """Test that flows must return a TaskNode."""

    @flow
    def bad_flow():
        return Ok("not a TaskNode")

    import pytest

    with pytest.raises(TypeError, match="must return a TaskNode"):
        bad_flow()


def test_direct_task_call_in_flow_raises():
    """Test that calling a task directly inside a flow raises."""

    @task
    def my_task() -> Result[str]:
        return Ok("done")

    @flow
    def bad_direct_flow():
        my_task()  # This should raise
        return my_task.flow()

    import pytest

    with pytest.raises(recompose.DirectTaskCallInFlowError):
        bad_direct_flow()
