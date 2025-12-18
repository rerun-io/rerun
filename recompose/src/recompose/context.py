"""Execution context for recompose tasks."""

from __future__ import annotations

from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from .automation import AutomationInfo
    from .flow import FlowInfo
    from .task import TaskInfo

# Debug mode flag
_debug_mode: bool = False

# Module name for subprocess isolation (set by main())
# This is the importable module path, e.g., "examples.app"
_module_name: str | None = None

# Python command for GHA workflow generation (e.g., "python", "uv run python")
_python_cmd: str = "python"

# Working directory for GHA workflow generation (relative to repo root)
_working_directory: str | None = None


@dataclass
class OutputLine:
    """A captured line of output."""

    level: Literal["out", "dbg"]
    message: str


@dataclass
class TaskContext:
    """
    Execution context for a single task.

    Tracks output and provides task metadata.
    """

    task_name: str
    output: list[OutputLine] = field(default_factory=list)

    def capture_out(self, message: str) -> None:
        """Capture an output line."""
        self.output.append(OutputLine(level="out", message=message))

    def capture_dbg(self, message: str) -> None:
        """Capture a debug line."""
        self.output.append(OutputLine(level="dbg", message=message))


# Backwards compatibility alias
Context = TaskContext


@dataclass
class RecomposeContext:
    """
    Global recompose execution context.

    Holds the registries of tasks, flows, and automations that were
    explicitly registered via main(). This replaces the global registries.
    """

    tasks: dict[str, TaskInfo] = field(default_factory=dict)
    flows: dict[str, FlowInfo] = field(default_factory=dict)
    automations: dict[str, AutomationInfo] = field(default_factory=dict)


# Context variable for the current task context (per-task)
_current_task_context: ContextVar[TaskContext | None] = ContextVar("recompose_task_context", default=None)

# Context variable for the global recompose context (set by main())
_recompose_context: ContextVar[RecomposeContext | None] = ContextVar("recompose_context", default=None)


def get_context() -> TaskContext | None:
    """Get the current task context, or None if not in a task."""
    return _current_task_context.get()


def set_context(ctx: TaskContext | None) -> None:
    """Set the current task context."""
    _current_task_context.set(ctx)


def get_recompose_context() -> RecomposeContext | None:
    """Get the global recompose context, or None if not running via main()."""
    return _recompose_context.get()


def set_recompose_context(ctx: RecomposeContext | None) -> None:
    """Set the global recompose context (called by main())."""
    _recompose_context.set(ctx)


def get_task_registry() -> dict[str, TaskInfo]:
    """
    Get the task registry from the current recompose context.

    Returns an empty dict if not running in a recompose context.
    """
    ctx = _recompose_context.get()
    if ctx is None:
        return {}
    return ctx.tasks


def get_flow_registry() -> dict[str, FlowInfo]:
    """
    Get the flow registry from the current recompose context.

    Returns an empty dict if not running in a recompose context.
    """
    ctx = _recompose_context.get()
    if ctx is None:
        return {}
    return ctx.flows


def get_automation_registry() -> dict[str, AutomationInfo]:
    """
    Get the automation registry from the current recompose context.

    Returns an empty dict if not running in a recompose context.
    """
    ctx = _recompose_context.get()
    if ctx is None:
        return {}
    return ctx.automations


def get_task(name: str) -> TaskInfo | None:
    """
    Look up a task by name.

    Args:
        name: Task name (short name or full module:name).

    Returns:
        TaskInfo if found, None otherwise.

    """
    registry = get_task_registry()

    # Try exact match first
    if name in registry:
        return registry[name]

    # Try short name match
    for full_name, info in registry.items():
        if info.name == name:
            return info

    return None


def get_flow(name: str) -> FlowInfo | None:
    """
    Look up a flow by name.

    Args:
        name: Flow name (short name or full module:name).

    Returns:
        FlowInfo if found, None otherwise.

    """
    registry = get_flow_registry()

    # Try exact match first
    if name in registry:
        return registry[name]

    # Try short name match
    for full_name, info in registry.items():
        if info.name == name:
            return info

    return None


def get_automation(name: str) -> AutomationInfo | None:
    """
    Look up an automation by name.

    Args:
        name: Automation name (short name or full module:name).

    Returns:
        AutomationInfo if found, None otherwise.

    """
    registry = get_automation_registry()

    # Try exact match first
    if name in registry:
        return registry[name]

    # Try short name match
    for full_name, info in registry.items():
        if info.name == name:
            return info

    return None


def set_debug(enabled: bool) -> None:
    """Enable or disable debug output."""
    global _debug_mode
    _debug_mode = enabled


def is_debug() -> bool:
    """Check if debug mode is enabled."""
    return _debug_mode


def set_module_name(name: str) -> None:
    """
    Set the module name for subprocess isolation (called by main()).

    Args:
        name: Importable module path (e.g., "examples.app")

    """
    global _module_name
    _module_name = name


def get_module_name() -> str | None:
    """
    Get the module name for subprocess isolation.

    Returns:
        Importable module path, or None if not set.

    """
    return _module_name


def set_python_cmd(cmd: str) -> None:
    """
    Set the Python command for GHA workflow generation.

    Args:
        cmd: Command to invoke Python (e.g., "python", "uv run python").

    """
    global _python_cmd
    _python_cmd = cmd


def get_python_cmd() -> str:
    """
    Get the Python command for GHA workflow generation.

    Returns:
        Command to invoke Python (default: "python").

    """
    return _python_cmd


def set_working_directory(directory: str | None) -> None:
    """
    Set the working directory for GHA workflow generation.

    Args:
        directory: Working directory relative to repo root, or None for repo root.

    """
    global _working_directory
    _working_directory = directory


def get_working_directory() -> str | None:
    """
    Get the working directory for GHA workflow generation.

    Returns:
        Working directory relative to repo root, or None for repo root.

    """
    return _working_directory


def out(message: str) -> None:
    """
    Output a message.

    When running inside a task context, the message is captured.
    Always prints to console. In tree mode, stdout is wrapped to add prefixes
    automatically, so we just print normally.
    """
    ctx = _current_task_context.get()
    if ctx is not None:
        ctx.capture_out(message)

    # Just print - if in tree mode, the TreePrefixWriter wrapper handles prefixing
    print(message, flush=True)


def dbg(message: str) -> None:
    """
    Output a debug message.

    When running inside a task context, the message is captured.
    Only prints to console if debug mode is enabled.
    In tree mode, stdout is wrapped to add prefixes automatically.
    """
    ctx = _current_task_context.get()
    if ctx is not None:
        ctx.capture_dbg(message)
    if _debug_mode:
        # Just print - if in tree mode, the TreePrefixWriter wrapper handles prefixing
        print(f"[debug] {message}", flush=True)
