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

from .cli import main
from .context import dbg, get_context, is_debug, out, set_debug
from .flow import (
    DirectTaskCallInFlowError,
    FlowContext,
    FlowInfo,
    FlowWrapper,
    flow,
    get_current_plan,
    get_flow,
    get_flow_context,
    get_flow_registry,
)
from .flowgraph import FlowPlan, TaskNode
from .result import Err, Ok, Result
from .subprocess import RunResult, SubprocessError, run
from .task import TaskInfo, TaskWrapper, get_registry, get_task, task, taskclass

__all__ = [
    # Result types
    "Result",
    "Ok",
    "Err",
    # Task decorator
    "task",
    "taskclass",
    "TaskInfo",
    "TaskWrapper",
    "get_registry",
    "get_task",
    # Context helpers
    "out",
    "dbg",
    "get_context",
    "set_debug",
    "is_debug",
    # Subprocess helpers
    "run",
    "RunResult",
    "SubprocessError",
    # Flow
    "flow",
    "FlowInfo",
    "FlowWrapper",
    "FlowContext",
    "FlowPlan",
    "TaskNode",
    "DirectTaskCallInFlowError",
    "get_flow",
    "get_flow_registry",
    "get_flow_context",
    "get_current_plan",
    # CLI
    "main",
]

__version__ = "0.1.0"
