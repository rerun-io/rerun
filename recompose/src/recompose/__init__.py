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

# Legacy flow-dispatch automation (to be removed in Phase 6)
from .automation import (
    AutomationInfo as LegacyAutomationInfo,
    AutomationPlan,
    FlowDispatch,
    automation as legacy_automation,
)

# New P14 job-based automation framework
from .jobs import (
    # Reference types
    ArtifactRef,
    JobOutputRef,
    InputParamRef,
    # Input types
    InputParam,
    Artifact,
    # Job specification
    JobSpec,
    job,
    # Condition expressions
    ConditionExpr,
    InputCondition,
    GitHubCondition,
    AndCondition,
    OrCondition,
    NotCondition,
    github,
    # Triggers
    Trigger,
    PushTrigger,
    PullRequestTrigger,
    ScheduleTrigger,
    WorkflowDispatchTrigger,
    on_push,
    on_pull_request,
    on_schedule,
    on_workflow_dispatch,
    # Automation
    AutomationInfo,
    AutomationWrapper,
    automation,
)
from .builtin_tasks import builtin_commands, generate_gha, inspect
from .cli import main
from .command_group import App, CommandGroup
from .conditional import run_if
from .context import (
    ArtifactInfo,
    dbg,
    get_automation,
    get_automation_registry,
    get_context,
    get_flow,
    get_flow_registry,
    get_python_cmd,
    get_secret,
    get_task,
    get_task_registry,
    get_working_directory,
    is_debug,
    out,
    save_artifact,
    set_debug,
    set_output,
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
from .step import step, step_decorator
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
    # Task outputs/artifacts/secrets
    "set_output",
    "save_artifact",
    "get_secret",
    "ArtifactInfo",
    # Step grouping
    "step",
    "step_decorator",
    # Subprocess helpers
    "run",
    "RunResult",
    "SubprocessError",
    # Flow (legacy - to be removed in Phase 6)
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
    "App",
    "CommandGroup",
    # Workspace (for subprocess isolation - legacy)
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
    # Legacy automations (to be removed in Phase 6)
    "LegacyAutomationInfo",
    "legacy_automation",
    "AutomationPlan",
    "FlowDispatch",
    # P14: Job-based automation framework
    "automation",
    "AutomationInfo",
    "AutomationWrapper",
    "get_automation",
    "get_automation_registry",
    # P14: Job types
    "job",
    "JobSpec",
    "JobOutputRef",
    "ArtifactRef",
    "InputParamRef",
    # P14: Input types
    "InputParam",
    "Artifact",
    # P14: Condition expressions
    "ConditionExpr",
    "InputCondition",
    "GitHubCondition",
    "AndCondition",
    "OrCondition",
    "NotCondition",
    "github",
    # P14: Triggers
    "Trigger",
    "PushTrigger",
    "PullRequestTrigger",
    "ScheduleTrigger",
    "WorkflowDispatchTrigger",
    "on_push",
    "on_pull_request",
    "on_schedule",
    "on_workflow_dispatch",
    # Built-in tasks
    "builtin_commands",
    "generate_gha",
    "inspect",
]

__version__ = "0.1.0"
