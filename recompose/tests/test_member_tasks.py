"""Tests for class-based member tasks."""

from recompose import Ok, Result, get_registry, task, taskclass


def test_taskclass_registers_method_tasks():
    """Test that @taskclass registers @task methods."""

    @taskclass
    class TestClass:
        def __init__(self, *, name: str):
            self.name = name

        @task
        def greet(self) -> Result[str]:
            return Ok(f"Hello, {self.name}!")

    registry = get_registry()
    assert any("testclass.greet" in key for key in registry)


def test_method_task_has_combined_signature():
    """Test that method tasks combine __init__ and method params."""

    @taskclass
    class Calculator:
        def __init__(self, *, base: int = 0):
            self.base = base

        @task
        def add(self, *, value: int) -> Result[int]:
            return Ok(self.base + value)

    registry = get_registry()
    task_info = None
    for key, info in registry.items():
        if "calculator.add" in key:
            task_info = info
            break

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

        @task
        def say(self, *, name: str) -> Result[str]:
            return Ok(f"{self.prefix}, {name}!")

    registry = get_registry()
    task_info = None
    for key, info in registry.items():
        if "greeter.say" in key:
            task_info = info
            break

    assert task_info is not None

    # Call the wrapper with combined args
    result = task_info.fn(prefix="Hi", name="World")
    assert result.ok
    assert result.value() == "Hi, World!"


def test_method_task_with_defaults():
    """Test method tasks with default arguments."""

    @taskclass
    class Counter:
        def __init__(self, *, start: int = 0):
            self.value = start

        @task
        def increment(self, *, by: int = 1) -> Result[int]:
            self.value += by
            return Ok(self.value)

    registry = get_registry()
    task_info = None
    for key, info in registry.items():
        if "counter.increment" in key:
            task_info = info
            break

    assert task_info is not None

    # Call with all defaults
    result = task_info.fn()
    assert result.ok
    assert result.value() == 1

    # Call with custom values
    result = task_info.fn(start=10, by=5)
    assert result.ok
    assert result.value() == 15


def test_method_task_exception_handling():
    """Test that exceptions in method tasks are caught."""

    @taskclass
    class Failer:
        def __init__(self):
            pass

        @task
        def fail(self) -> Result[None]:
            raise ValueError("Intentional failure")

    registry = get_registry()
    task_info = None
    for key, info in registry.items():
        if "failer.fail" in key:
            task_info = info
            break

    assert task_info is not None

    result = task_info.fn()
    assert result.failed
    assert "ValueError" in result.error
    assert "Intentional failure" in result.error


def test_multiple_method_tasks():
    """Test class with multiple @task methods."""

    @taskclass
    class MultiTask:
        def __init__(self, *, name: str):
            self.name = name

        @task
        def first(self) -> Result[str]:
            return Ok(f"first: {self.name}")

        @task
        def second(self, *, extra: str = "") -> Result[str]:
            return Ok(f"second: {self.name} {extra}")

    registry = get_registry()

    first_info = None
    second_info = None
    for key, info in registry.items():
        if "multitask.first" in key:
            first_info = info
        if "multitask.second" in key:
            second_info = info

    assert first_info is not None
    assert second_info is not None

    # Call first
    result = first_info.fn(name="test")
    assert result.ok
    assert result.value() == "first: test"

    # Call second
    result = second_info.fn(name="test", extra="!")
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


def test_method_task_preserves_docstring():
    """Test that method docstrings are preserved."""

    @taskclass
    class Documented:
        def __init__(self):
            pass

        @task
        def documented_method(self) -> Result[None]:
            """This is the docstring."""
            return Ok(None)

    registry = get_registry()
    task_info = None
    for key, info in registry.items():
        if "documented.documented_method" in key:
            task_info = info
            break

    assert task_info is not None
    assert task_info.doc == "This is the docstring."
