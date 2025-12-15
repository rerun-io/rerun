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
    print(result.value())  # "greeted World"

    # Or use the CLI:
    recompose.main()
"""

from . import gha
from .automation import (
    AutomationInfo,
    AutomationPlan,
    FlowDispatch,
    automation,
    get_automation,
    get_automation_registry,
)
from .builtin_tasks import generate_gha, inspect
from .cli import main
from .context import (
    dbg,
    get_context,
    get_python_cmd,
    get_working_directory,
    is_debug,
    out,
    set_debug,
    set_python_cmd,
    set_working_directory,
)
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
from .flowgraph import FlowPlan, Input, InputPlaceholder, TaskNode
from .result import Err, Ok, Result
from .subprocess import RunResult, SubprocessError, run
from .task import TaskInfo, TaskWrapper, get_registry, get_task, task, taskclass
from .workspace import FlowParams, create_workspace, read_params, read_step_result, write_params, write_step_result

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
    "get_python_cmd",
    "set_python_cmd",
    "get_working_directory",
    "set_working_directory",
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
    "Input",
    "InputPlaceholder",
    "DirectTaskCallInFlowError",
    "get_flow",
    "get_flow_registry",
    "get_flow_context",
    "get_current_plan",
    # CLI
    "main",
    # Workspace (for subprocess isolation)
    "FlowParams",
    "create_workspace",
    "write_params",
    "read_params",
    "write_step_result",
    "read_step_result",
    # GHA generation
    "gha",
    # Automations
    "automation",
    "AutomationInfo",
    "AutomationPlan",
    "FlowDispatch",
    "get_automation",
    "get_automation_registry",
    # Built-in tasks
    "generate_gha",
    "inspect",
]

__version__ = "0.1.0"
