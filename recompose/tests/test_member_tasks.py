"""Tests for class-based member tasks."""

from recompose import Ok, Result, method, task, taskclass


def test_taskclass_creates_recompose_tasks():
    """Test that @taskclass creates _recompose_tasks dict."""

    @taskclass
    class TestClass:
        def __init__(self, *, name: str):
            self.name = name

        @method
        def greet(self) -> Result[str]:
            return Ok(f"Hello, {self.name}!")

    assert hasattr(TestClass, "_recompose_tasks")
    assert "greet" in TestClass._recompose_tasks


def test_method_task_has_combined_signature():
    """Test that method tasks combine __init__ and method params."""

    @taskclass
    class Calculator:
        def __init__(self, *, base: int = 0):
            self.base = base

        @method
        def add(self, *, value: int) -> Result[int]:
            return Ok(self.base + value)

    wrapper = Calculator._recompose_tasks["add"]
    task_info = wrapper._task_info

    assert task_info is not None
    assert task_info.is_method
    assert task_info.cls is Calculator
    assert task_info.method_name == "add"

    # Check combined signature has both 'base' and 'value'
    param_names = list(task_info.signature.parameters.keys())
    assert "base" in param_names
    assert "value" in param_names


def test_method_task_can_be_invoked():
    """Test that method tasks can be called via the wrapper."""

    @taskclass
    class Greeter:
        def __init__(self, *, prefix: str = "Hello"):
            self.prefix = prefix

        @method
        def say(self, *, name: str) -> Result[str]:
            return Ok(f"{self.prefix}, {name}!")

    wrapper = Greeter._recompose_tasks["say"]

    # Call the wrapper with combined args
    result = wrapper(prefix="Hi", name="World")
    assert result.ok
    assert result.value() == "Hi, World!"


def test_method_task_with_defaults():
    """Test method tasks with default arguments."""

    @taskclass
    class Counter:
        def __init__(self, *, start: int = 0):
            self.value = start

        @method
        def increment(self, *, by: int = 1) -> Result[int]:
            self.value += by
            return Ok(self.value)

    wrapper = Counter._recompose_tasks["increment"]

    # Call with all defaults
    result = wrapper()
    assert result.ok
    assert result.value() == 1

    # Call with custom values
    result = wrapper(start=10, by=5)
    assert result.ok
    assert result.value() == 15


def test_method_task_exception_handling():
    """Test that exceptions in method tasks are caught."""

    @taskclass
    class Failer:
        def __init__(self):
            pass

        @method
        def fail(self) -> Result[None]:
            raise ValueError("Intentional failure")

    wrapper = Failer._recompose_tasks["fail"]

    result = wrapper()
    assert result.failed
    assert "ValueError" in result.error
    assert "Intentional failure" in result.error


def test_multiple_method_tasks():
    """Test class with multiple @method methods."""

    @taskclass
    class MultiTask:
        def __init__(self, *, name: str):
            self.name = name

        @method
        def first(self) -> Result[str]:
            return Ok(f"first: {self.name}")

        @method
        def second(self, *, extra: str = "") -> Result[str]:
            return Ok(f"second: {self.name} {extra}")

    assert "first" in MultiTask._recompose_tasks
    assert "second" in MultiTask._recompose_tasks

    first_wrapper = MultiTask._recompose_tasks["first"]
    second_wrapper = MultiTask._recompose_tasks["second"]

    # Call first
    result = first_wrapper(name="test")
    assert result.ok
    assert result.value() == "first: test"

    # Call second
    result = second_wrapper(name="test", extra="!")
    assert result.ok
    assert result.value() == "second: test !"


def test_task_decorator_still_works_for_functions():
    """Ensure @task still works normally for standalone functions."""

    @task
    def standalone(*, value: int) -> Result[int]:
        return Ok(value * 2)

    result = standalone(value=21)
    assert result.ok
    assert result.value() == 42


def test_method_decorator_preserves_docstring():
    """Test that method docstrings are preserved."""

    @taskclass
    class Documented:
        def __init__(self):
            pass

        @method
        def documented_method(self) -> Result[None]:
            """This is the docstring."""
            return Ok(None)

    wrapper = Documented._recompose_tasks["documented_method"]
    task_info = wrapper._task_info

    assert task_info is not None
    assert task_info.doc == "This is the docstring."


def test_task_decorator_errors_on_methods():
    """Test that @task raises an error when used on methods."""
    import pytest

    with pytest.raises(TypeError, match="@task cannot be used on methods"):

        @task
        def method_with_self(self, *, value: int) -> Result[int]:
            return Ok(value)


def test_method_decorator_errors_on_functions():
    """Test that @method raises an error when used on non-methods."""
    import pytest

    with pytest.raises(TypeError, match="@method can only be used on methods"):

        @method
        def function_without_self(*, value: int) -> Result[int]:
            return Ok(value)
