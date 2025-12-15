"""Tests for the @task decorator."""

from recompose import Ok, Result, get_registry, task


def test_task_registers_function():
    @task
    def my_test_task() -> Result[str]:
        return Ok("done")

    registry = get_registry()
    assert any("my_test_task" in key for key in registry)


def test_task_returns_result():
    @task
    def simple_task() -> Result[int]:
        return Ok(42)

    result = simple_task()
    assert result.ok
    assert result.value == 42


def test_task_with_arguments():
    @task
    def add_task(*, a: int, b: int) -> Result[int]:
        return Ok(a + b)

    result = add_task(a=2, b=3)
    assert result.ok
    assert result.value == 5


def test_task_with_default_arguments():
    @task
    def greet_task(*, name: str, greeting: str = "Hello") -> Result[str]:
        return Ok(f"{greeting}, {name}!")

    result = greet_task(name="World")
    assert result.ok
    assert result.value == "Hello, World!"

    result2 = greet_task(name="World", greeting="Hi")
    assert result2.ok
    assert result2.value == "Hi, World!"


def test_task_catches_exceptions():
    @task
    def failing_task() -> Result[str]:
        raise ValueError("Something went wrong!")

    result = failing_task()
    assert result.failed
    assert "ValueError: Something went wrong!" in result.error
    assert result.traceback is not None


def test_task_wraps_non_result_return():
    @task
    def non_result_task() -> Result[int]:
        return 42  # type: ignore[return-value]  # intentionally returning wrong type

    result = non_result_task()
    assert result.ok
    assert result.value == 42


def test_task_preserves_docstring():
    @task
    def documented_task() -> Result[str]:
        """This is a documented task."""
        return Ok("done")

    assert documented_task.__doc__ == "This is a documented task."


def test_task_info_attached():
    @task
    def info_task() -> Result[str]:
        return Ok("done")

    assert hasattr(info_task, "_task_info")
    assert info_task._task_info.name == "info_task"
