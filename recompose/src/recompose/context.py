"""Execution context for recompose tasks."""

from __future__ import annotations

import os
from contextvars import ContextVar
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any, Literal

if TYPE_CHECKING:
    from .jobs import JobSpec
    from .task import TaskInfo

# Debug mode flag
_debug_mode: bool = False

# Module name for subprocess isolation (set by main())
# This is the importable module path, e.g., "examples.app"
_module_name: str | None = None

# CLI command for GHA workflow generation (e.g., "./run", "uv run python -m app")
_cli_command: str = "./run"

# Working directory for GHA workflow generation (relative to repo root)
_working_directory: str | None = None


@dataclass
class OutputLine:
    """A captured line of output."""

    level: Literal["out", "dbg"]
    message: str


@dataclass
class ArtifactInfo:
    """Information about a saved artifact."""

    name: str
    path: Path


@dataclass
class TaskContext:
    """
    Execution context for a single task.

    Tracks output and provides task metadata.
    """

    task_name: str
    output: list[OutputLine] = field(default_factory=list)

    # P14: Task declarations for validation
    declared_outputs: list[str] = field(default_factory=list)
    declared_artifacts: list[str] = field(default_factory=list)
    declared_secrets: list[str] = field(default_factory=list)

    # P14: Collected outputs and artifacts
    task_outputs: dict[str, str] = field(default_factory=dict)
    task_artifacts: dict[str, ArtifactInfo] = field(default_factory=dict)

    def capture_out(self, message: str) -> None:
        """Capture an output line."""
        self.output.append(OutputLine(level="out", message=message))

    def capture_dbg(self, message: str) -> None:
        """Capture a debug line."""
        self.output.append(OutputLine(level="dbg", message=message))

    def set_output(self, name: str, value: str) -> None:
        """
        Set a task output value.

        Validates the name against declared outputs and writes to GITHUB_OUTPUT if in GHA.
        """
        if self.declared_outputs and name not in self.declared_outputs:
            raise ValueError(
                f"Output '{name}' not declared in @task(outputs=[...]). Declared outputs: {self.declared_outputs}"
            )
        self.task_outputs[name] = value

        # Write to GITHUB_OUTPUT if running in GHA
        github_output = os.environ.get("GITHUB_OUTPUT")
        if github_output:
            with open(github_output, "a") as f:
                # Handle multi-line values with delimiter syntax
                if "\n" in value:
                    import uuid

                    delimiter = f"ghadelimiter_{uuid.uuid4()}"
                    f.write(f"{name}<<{delimiter}\n{value}\n{delimiter}\n")
                else:
                    f.write(f"{name}={value}\n")

    def save_artifact(self, name: str, path: Path) -> None:
        """
        Save an artifact.

        Validates the name against declared artifacts.
        """
        if self.declared_artifacts and name not in self.declared_artifacts:
            raise ValueError(
                f"Artifact '{name}' not declared in @task(artifacts=[...]). "
                f"Declared artifacts: {self.declared_artifacts}"
            )
        if not path.exists():
            raise FileNotFoundError(f"Artifact path does not exist: {path}")
        self.task_artifacts[name] = ArtifactInfo(name=name, path=path)

    def get_secret(self, name: str) -> str:
        """
        Get a secret value.

        Validates the name against declared secrets.
        In GHA, reads from environment (set by workflow).
        Locally, reads from ~/.recompose/secrets.toml.
        """
        if self.declared_secrets and name not in self.declared_secrets:
            raise ValueError(
                f"Secret '{name}' not declared in @task(secrets=[...]). Declared secrets: {self.declared_secrets}"
            )

        # First try environment variable (GHA or explicit local)
        value = os.environ.get(name)
        if value is not None:
            return value

        # Fall back to local secrets file
        secrets_file = Path.home() / ".recompose" / "secrets.toml"
        if secrets_file.exists():
            try:
                import tomllib

                with open(secrets_file, "rb") as f:
                    secrets = tomllib.load(f)
                if name in secrets:
                    return str(secrets[name])
            except Exception as e:
                raise RuntimeError(f"Failed to read secrets file {secrets_file}: {e}") from e

        raise ValueError(f"Secret '{name}' not found. Set as environment variable or add to ~/.recompose/secrets.toml")


# Backwards compatibility alias
Context = TaskContext


@dataclass
class AutomationContext:
    """
    Context for building an automation plan.

    Tracks jobs created via recompose.job() calls during @automation execution.
    """

    automation_name: str
    """Name of the automation being built."""

    jobs: list[JobSpec] = field(default_factory=list)
    """Jobs created during automation execution."""

    input_params: dict[str, Any] = field(default_factory=dict)
    """Input parameter values (for local execution)."""

    def add_job(self, job_spec: JobSpec) -> None:
        """Add a job to this automation."""
        self.jobs.append(job_spec)


# Context variable for the current automation context
_current_automation_context: ContextVar[AutomationContext | None] = ContextVar(
    "recompose_automation_context", default=None
)


def get_automation_context() -> AutomationContext | None:
    """Get the current automation context, or None if not in an automation."""
    return _current_automation_context.get()


def set_automation_context(ctx: AutomationContext | None) -> None:
    """Set the current automation context."""
    _current_automation_context.set(ctx)


@dataclass
class RecomposeContext:
    """
    Global recompose execution context.

    Holds the registries of tasks and automations that were
    explicitly registered via main(). This replaces the global registries.

    Note: Dispatchables (from make_dispatchable) are automations with
    workflow_dispatch trigger, so they go in the automations registry.
    """

    tasks: dict[str, TaskInfo] = field(default_factory=dict)
    automations: dict[str, Any] = field(default_factory=dict)  # AutomationInfo


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


def get_automation_registry() -> dict[str, Any]:
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


def get_automation(name: str) -> Any | None:
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


def set_cli_command(cmd: str) -> None:
    """
    Set the CLI command for GHA workflow generation.

    Args:
        cmd: CLI entry point command (e.g., "./run", "uv run python -m app").

    """
    global _cli_command
    _cli_command = cmd


def get_cli_command() -> str:
    """
    Get the CLI command for GHA workflow generation.

    Returns:
        CLI entry point command (default: "./run").

    """
    return _cli_command


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


def set_output(name: str, value: str) -> None:
    """
    Set a task output value.

    Must be called from within a task that declared the output in @task(outputs=[...]).
    In GHA, also writes to GITHUB_OUTPUT file.

    Args:
        name: Output name (must be declared in @task decorator)
        value: Output value (will be converted to string)

    Raises:
        RuntimeError: If not called from within a task context
        ValueError: If name is not in declared outputs

    Example:
        @task(outputs=["wheel_path", "version"])
        def build_wheel() -> Result[None]:
            recompose.set_output("wheel_path", "/dist/pkg-1.0.0.whl")
            recompose.set_output("version", "1.0.0")
            return Ok(None)

    """
    ctx = _current_task_context.get()
    if ctx is None:
        raise RuntimeError("set_output() must be called from within a task context")
    ctx.set_output(name, str(value))


def save_artifact(name: str, path: Path | str) -> None:
    """
    Save an artifact file.

    Must be called from within a task that declared the artifact in @task(artifacts=[...]).
    In GHA automation, this triggers upload-artifact after the task completes.

    Args:
        name: Artifact name (must be declared in @task decorator)
        path: Path to the artifact file or directory

    Raises:
        RuntimeError: If not called from within a task context
        ValueError: If name is not in declared artifacts
        FileNotFoundError: If path does not exist

    Example:
        @task(artifacts=["wheel"])
        def build_wheel() -> Result[None]:
            run("uv", "build", "--wheel")
            recompose.save_artifact("wheel", Path("dist/pkg-1.0.0.whl"))
            return Ok(None)

    """
    ctx = _current_task_context.get()
    if ctx is None:
        raise RuntimeError("save_artifact() must be called from within a task context")
    ctx.save_artifact(name, Path(path) if isinstance(path, str) else path)


def get_secret(name: str) -> str:
    """
    Get a secret value.

    Must be called from within a task that declared the secret in @task(secrets=[...]).
    In GHA, reads from environment (secrets are passed via workflow env).
    Locally, reads from ~/.recompose/secrets.toml.

    Args:
        name: Secret name (must be declared in @task decorator)

    Returns:
        The secret value

    Raises:
        RuntimeError: If not called from within a task context
        ValueError: If name is not in declared secrets, or secret not found

    Example:
        @task(secrets=["PYPI_TOKEN"])
        def publish() -> Result[None]:
            token = recompose.get_secret("PYPI_TOKEN")
            # use token...
            return Ok(None)

    """
    ctx = _current_task_context.get()
    if ctx is None:
        raise RuntimeError("get_secret() must be called from within a task context")
    return ctx.get_secret(name)
