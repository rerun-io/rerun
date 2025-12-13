"""
Recompose - A lightweight, typed, pythonic task execution framework.

Basic usage:

    import recompose

    @recompose.task
    def greet(*, name: str) -> recompose.Result[str]:
        recompose.out(f"Hello, {name}!")
        return recompose.Ok(f"greeted {name}")

    # Call directly as a function:
    result = greet(name="World")
    assert result.ok
    print(result.value)  # "greeted World"

    # Or use the CLI:
    recompose.main()
"""

from .context import dbg, get_context, is_debug, out, set_debug
from .result import Err, Ok, Result
from .task import TaskInfo, get_registry, get_task, task

__all__ = [
    # Result types
    "Result",
    "Ok",
    "Err",
    # Task decorator
    "task",
    "TaskInfo",
    "get_registry",
    "get_task",
    # Context helpers
    "out",
    "dbg",
    "get_context",
    "set_debug",
    "is_debug",
]

__version__ = "0.1.0"
