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

from . import gh_cli, gha
from .automation import (
    AutomationInfo,
    AutomationPlan,
    FlowDispatch,
    automation,
)
from .builtin_tasks import builtin_commands, generate_gha, inspect
from .cli import main
from .command_group import CommandGroup, Config
from .conditional import run_if
from .context import (
    dbg,
    get_automation,
    get_automation_registry,
    get_context,
    get_flow,
    get_flow_registry,
    get_python_cmd,
    get_task,
    get_task_registry,
    get_working_directory,
    is_debug,
    out,
    set_debug,
    set_python_cmd,
    set_working_directory,
)
from .flow import (
    FlowInfo,
    FlowWrapper,
    flow,
    get_current_plan,
)
from .plan import FlowPlan, Input, InputPlaceholder, TaskClassNode, TaskNode
from .result import Err, Ok, Result
from .subprocess import RunResult, SubprocessError, run
from .task import MethodWrapper, TaskInfo, TaskWrapper, method, task, taskclass
from .workspace import (
    FlowParams,
    Serializer,
    create_workspace,
    read_params,
    read_step_result,
    read_taskclass_state,
    register_serializer,
    write_params,
    write_step_result,
    write_taskclass_state,
)

__all__ = [
    # Result types
    "Result",
    "Ok",
    "Err",
    # Task decorator
    "task",
    "taskclass",
    "method",
    "MethodWrapper",
    "TaskInfo",
    "TaskWrapper",
    "get_task_registry",
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
    "FlowPlan",
    "TaskNode",
    "TaskClassNode",
    "Input",
    "InputPlaceholder",
    "get_flow",
    "get_flow_registry",
    "get_current_plan",
    # Conditional execution
    "run_if",
    # CLI
    "main",
    "Config",
    "CommandGroup",
    # Workspace (for subprocess isolation)
    "FlowParams",
    "create_workspace",
    "write_params",
    "read_params",
    "write_step_result",
    "read_step_result",
    "write_taskclass_state",
    "read_taskclass_state",
    # Serialization
    "Serializer",
    "register_serializer",
    # GHA generation
    "gha",
    # GitHub CLI integration
    "gh_cli",
    # Automations
    "automation",
    "AutomationInfo",
    "AutomationPlan",
    "FlowDispatch",
    "get_automation",
    "get_automation_registry",
    # Built-in tasks
    "builtin_commands",
    "generate_gha",
    "inspect",
]

__version__ = "0.1.0"
