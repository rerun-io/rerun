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
from .builtin_tasks import builtin_commands, generate_gha, inspect
from .cli import main
from .command_group import App, CommandGroup
from .context import (
    ArtifactInfo,
    dbg,
    get_automation,
    get_automation_registry,
    get_cli_command,
    get_context,
    get_secret,
    get_task,
    get_task_registry,
    get_working_directory,
    is_debug,
    out,
    save_artifact,
    set_cli_command,
    set_debug,
    set_output,
    set_working_directory,
)

# P14 job-based automation framework
from .jobs import (
    AndCondition,
    Artifact,
    # Reference types
    ArtifactRef,
    # Automation
    AutomationInfo,
    AutomationWrapper,
    # P14 Phase 5: Dispatchable
    BoolInput,
    ChoiceInput,
    CombinedTrigger,
    # Condition expressions
    ConditionExpr,
    Dispatchable,
    DispatchableInfo,
    DispatchInput,
    GitHubCondition,
    InputCondition,
    # Input types
    InputParam,
    InputParamRef,
    JobOutputRef,
    # Job specification
    JobSpec,
    NotCondition,
    OrCondition,
    PullRequestTrigger,
    PushTrigger,
    ScheduleTrigger,
    StringInput,
    # Triggers
    Trigger,
    WorkflowDispatchTrigger,
    automation,
    github,
    job,
    make_dispatchable,
    on_pull_request,
    on_push,
    on_schedule,
    on_workflow_dispatch,
)
from .result import Err, Ok, Result
from .step import step, step_decorator
from .subprocess import RunResult, SubprocessError, run
from .task import TaskInfo, TaskWrapper, task

__all__ = [
    # Result types
    "Result",
    "Ok",
    "Err",
    # Task decorator
    "task",
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
    "get_cli_command",
    "set_cli_command",
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
    # CLI
    "main",
    "App",
    "CommandGroup",
    # GHA generation
    "gha",
    # GitHub CLI integration
    "gh_cli",
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
    "CombinedTrigger",
    "PushTrigger",
    "PullRequestTrigger",
    "ScheduleTrigger",
    "WorkflowDispatchTrigger",
    "on_push",
    "on_pull_request",
    "on_schedule",
    "on_workflow_dispatch",
    # P14 Phase 5: Dispatchable
    "make_dispatchable",
    "Dispatchable",
    "DispatchableInfo",
    "DispatchInput",
    "StringInput",
    "BoolInput",
    "ChoiceInput",
    # Built-in tasks
    "builtin_commands",
    "generate_gha",
    "inspect",
]

__version__ = "0.1.0"
