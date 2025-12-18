"""Job-based automation framework for orchestrating multi-job workflows.

This module implements the new "tasks as jobs" model where:
- Each task maps to a GHA job (not step)
- Automations orchestrate multiple jobs with `needs:` dependencies
- Dependencies are inferred from output/artifact references

Example:
    @recompose.automation
    def ci() -> None:
        lint_job = recompose.job(lint)
        build_job = recompose.job(build_wheel)

        # Dependency inferred from output reference
        test_job = recompose.job(
            test_wheel,
            inputs={"wheel_path": build_job.get("wheel_path")},
        )

"""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any, Generic, TypeVar, overload

if TYPE_CHECKING:
    from .task import TaskInfo, TaskWrapper

T = TypeVar("T")


# =============================================================================
# Reference Types (for dependency tracking)
# =============================================================================


@dataclass(frozen=True)
class JobOutputRef:
    """Reference to a job's output value.

    Created by JobSpec.get(output_name). When used in another job's inputs,
    this creates an implicit dependency and maps to GHA job output syntax.

    Example:
        build_job = job(build_wheel)
        test_job = job(test, inputs={"path": build_job.get("wheel_path")})

    In GHA this generates:
        jobs:
          build_wheel:
            outputs:
              wheel_path: ${{ steps.run.outputs.wheel_path }}
          test:
            needs: [build_wheel]
            steps:
              - run: ./run test --path="${{ needs.build_wheel.outputs.wheel_path }}"

    """

    job_id: str
    """ID of the job this reference points to."""

    output_name: str
    """Name of the output being referenced."""

    def __repr__(self) -> str:
        return f"JobOutputRef({self.job_id}.{self.output_name})"

    def to_gha_expr(self) -> str:
        """Convert to GitHub Actions expression syntax."""
        return f"${{{{ needs.{self.job_id}.outputs.{self.output_name} }}}}"


@dataclass(frozen=True)
class ArtifactRef:
    """Reference to a job's artifact.

    Created by JobSpec.artifact(artifact_name). When used in another job's inputs,
    this creates an implicit dependency and generates upload/download steps.

    Example:
        build_job = job(build_wheel, artifacts=["wheel"])
        test_job = job(test, inputs={"wheel": build_job.artifact("wheel")})

    In GHA this generates upload-artifact after build_wheel and
    download-artifact before test.

    """

    job_id: str
    """ID of the job that produces this artifact."""

    artifact_name: str
    """Name of the artifact being referenced."""

    def __repr__(self) -> str:
        return f"ArtifactRef({self.job_id}.{self.artifact_name})"


@dataclass(frozen=True)
class InputParamRef:
    """Reference to an automation input parameter.

    Created when InputParam values are used in job inputs.
    Maps to ${{ inputs.name }} in GHA.
    """

    param_name: str
    """Name of the input parameter."""

    def __repr__(self) -> str:
        return f"InputParamRef({self.param_name})"

    def to_gha_expr(self) -> str:
        """Convert to GitHub Actions expression syntax."""
        return f"${{{{ inputs.{self.param_name} }}}}"


# =============================================================================
# Input Parameter Types (for workflow_dispatch)
# =============================================================================


class InputParam(Generic[T]):
    """Marker type for automation input parameters.

    When used as a type hint in an @automation function, this declares
    a workflow_dispatch input. The value can be passed to jobs and
    referenced in conditions.

    Example:
        @automation
        def deploy(
            environment: InputParam[str],
            skip_tests: InputParam[bool] = InputParam(default=False),
        ) -> None:
            test_job = job(test, condition=~skip_tests)
            deploy_job = job(deploy_task, inputs={"env": environment})

    """

    def __init__(
        self,
        *,
        default: T | None = None,
        description: str | None = None,
        required: bool | None = None,
        choices: list[T] | None = None,
    ):
        self._default = default
        self._description = description
        self._required = required if required is not None else (default is None)
        self._choices = choices
        self._name: str | None = None  # Set when automation is decorated
        self._value: T | None = None  # Set when automation is called

    @property
    def name(self) -> str:
        """Get the parameter name (set during decoration)."""
        if self._name is None:
            raise RuntimeError("InputParam name not set - use inside @automation")
        return self._name

    def _set_name(self, name: str) -> None:
        """Set the parameter name (called by @automation decorator)."""
        self._name = name

    def _set_value(self, value: T) -> None:
        """Set the runtime value (called when automation executes)."""
        self._value = value

    def to_ref(self) -> InputParamRef:
        """Convert to a reference for use in job inputs."""
        return InputParamRef(self.name)

    # Expression algebra for conditions
    def __eq__(self, other: object) -> ConditionExpr:  # type: ignore[override]
        """Equality comparison for conditions."""
        return InputCondition(self.name, "==", other)

    def __ne__(self, other: object) -> ConditionExpr:  # type: ignore[override]
        """Inequality comparison for conditions."""
        return InputCondition(self.name, "!=", other)

    def __invert__(self) -> ConditionExpr:
        """Negation for boolean params (~param)."""
        return NotCondition(InputCondition(self.name, "==", True))

    def __bool__(self) -> bool:
        """Raise error - use conditions instead of Python control flow."""
        raise TypeError(
            "InputParam cannot be used in Python control flow.\nUse job(..., condition=param == 'value') instead."
        )

    def __repr__(self) -> str:
        name = self._name or "?"
        return f"InputParam({name})"


# =============================================================================
# Artifact Input Type
# =============================================================================


class Artifact:
    """Type hint for artifact inputs to tasks.

    When a task parameter is typed as Artifact, it indicates that this
    input should receive an artifact path. In GHA, this triggers
    download-artifact before the task runs.

    Example:
        @task
        def test_wheel(wheel: Artifact) -> Result[None]:
            # wheel is a Path to the downloaded artifact
            run("pip", "install", str(wheel))
            return Ok(None)

    """

    def __init__(self, path: Path | str | None = None):
        self._path = Path(path) if path else None

    @property
    def path(self) -> Path:
        """Get the artifact path."""
        if self._path is None:
            raise RuntimeError("Artifact path not set")
        return self._path

    def __fspath__(self) -> str:
        """Support os.fspath() for path-like usage."""
        return str(self.path)

    def __str__(self) -> str:
        return str(self.path) if self._path else "Artifact(?)"

    def __repr__(self) -> str:
        return f"Artifact({self._path})"


# =============================================================================
# Condition Expressions (for job-level if:)
# =============================================================================


class ConditionExpr:
    """Base class for job condition expressions.

    Conditions can be combined with & (and), | (or), and ~ (not).
    They map to GHA job-level `if:` expressions.
    """

    def __and__(self, other: ConditionExpr) -> AndCondition:
        """Logical AND."""
        return AndCondition(self, other)

    def __or__(self, other: ConditionExpr) -> OrCondition:
        """Logical OR."""
        return OrCondition(self, other)

    def __invert__(self) -> NotCondition:
        """Logical NOT."""
        return NotCondition(self)

    def __bool__(self) -> bool:
        """Raise error - expressions can't be used in Python control flow."""
        raise TypeError(
            "Condition expressions cannot be used in Python control flow.\n"
            "Use job(..., condition=expr) to set job conditions."
        )

    def to_gha_expr(self) -> str:
        """Convert to GitHub Actions expression syntax."""
        raise NotImplementedError

    def evaluate(self, context: dict[str, Any]) -> bool:
        """Evaluate the condition given runtime context."""
        raise NotImplementedError


@dataclass
class InputCondition(ConditionExpr):
    """Condition comparing an input parameter to a value."""

    param_name: str
    op: str  # "==" or "!="
    value: Any

    def to_gha_expr(self) -> str:
        if isinstance(self.value, bool):
            val_str = "true" if self.value else "false"
        elif isinstance(self.value, str):
            val_str = f"'{self.value}'"
        else:
            val_str = str(self.value)
        return f"inputs.{self.param_name} {self.op} {val_str}"

    def evaluate(self, context: dict[str, Any]) -> bool:
        inputs = context.get("inputs", {})
        actual = inputs.get(self.param_name)
        if self.op == "==":
            return actual == self.value
        elif self.op == "!=":
            return actual != self.value
        raise ValueError(f"Unknown operator: {self.op}")

    def __repr__(self) -> str:
        return f"({self.param_name} {self.op} {self.value!r})"


@dataclass
class GitHubCondition(ConditionExpr):
    """Condition referencing GitHub context."""

    context_path: str  # e.g., "github.ref_name", "github.event_name"
    op: str | None = None
    value: Any = None

    def eq(self, value: Any) -> GitHubCondition:
        """Create equality condition."""
        return GitHubCondition(self.context_path, "==", value)

    def ne(self, value: Any) -> GitHubCondition:
        """Create inequality condition."""
        return GitHubCondition(self.context_path, "!=", value)

    def __eq__(self, other: object) -> GitHubCondition:  # type: ignore[override]
        """Equality comparison."""
        return self.eq(other)

    def __ne__(self, other: object) -> GitHubCondition:  # type: ignore[override]
        """Inequality comparison."""
        return self.ne(other)

    def to_gha_expr(self) -> str:
        if self.op is None:
            return self.context_path
        if isinstance(self.value, bool):
            val_str = "true" if self.value else "false"
        elif isinstance(self.value, str):
            val_str = f"'{self.value}'"
        else:
            val_str = str(self.value)
        return f"{self.context_path} {self.op} {val_str}"

    def evaluate(self, context: dict[str, Any]) -> bool:
        # Parse the context path (e.g., "github.ref_name")
        parts = self.context_path.split(".")
        value = context
        for part in parts:
            value = value.get(part, {}) if isinstance(value, dict) else None

        if self.op is None:
            return bool(value)
        elif self.op == "==":
            return value == self.value
        elif self.op == "!=":
            return value != self.value
        raise ValueError(f"Unknown operator: {self.op}")

    def __repr__(self) -> str:
        if self.op:
            return f"({self.context_path} {self.op} {self.value!r})"
        return self.context_path


@dataclass
class AndCondition(ConditionExpr):
    """Logical AND of two conditions."""

    left: ConditionExpr
    right: ConditionExpr

    def to_gha_expr(self) -> str:
        return f"({self.left.to_gha_expr()}) && ({self.right.to_gha_expr()})"

    def evaluate(self, context: dict[str, Any]) -> bool:
        return self.left.evaluate(context) and self.right.evaluate(context)

    def __repr__(self) -> str:
        return f"({self.left!r} & {self.right!r})"


@dataclass
class OrCondition(ConditionExpr):
    """Logical OR of two conditions."""

    left: ConditionExpr
    right: ConditionExpr

    def to_gha_expr(self) -> str:
        return f"({self.left.to_gha_expr()}) || ({self.right.to_gha_expr()})"

    def evaluate(self, context: dict[str, Any]) -> bool:
        return self.left.evaluate(context) or self.right.evaluate(context)

    def __repr__(self) -> str:
        return f"({self.left!r} | {self.right!r})"


@dataclass
class NotCondition(ConditionExpr):
    """Logical NOT of a condition."""

    operand: ConditionExpr

    def to_gha_expr(self) -> str:
        return f"!({self.operand.to_gha_expr()})"

    def evaluate(self, context: dict[str, Any]) -> bool:
        return not self.operand.evaluate(context)

    def __repr__(self) -> str:
        return f"(~{self.operand!r})"


# =============================================================================
# GitHub Context References
# =============================================================================


class _GitHubContext:
    """Namespace for GitHub context references.

    Use these in conditions to reference GitHub Actions context values.

    Example:
        @automation
        def deploy(env: InputParam[str]) -> None:
            deploy_job = job(
                deploy_task,
                condition=(env == "prod") & github.ref_name.eq("main"),
            )

    """

    @property
    def event_name(self) -> GitHubCondition:
        """The event that triggered the workflow (e.g., 'push', 'pull_request')."""
        return GitHubCondition("github.event_name")

    @property
    def ref(self) -> GitHubCondition:
        """The full ref (e.g., 'refs/heads/main')."""
        return GitHubCondition("github.ref")

    @property
    def ref_name(self) -> GitHubCondition:
        """The short ref name (e.g., 'main')."""
        return GitHubCondition("github.ref_name")

    @property
    def ref_type(self) -> GitHubCondition:
        """The type of ref ('branch' or 'tag')."""
        return GitHubCondition("github.ref_type")

    @property
    def repository(self) -> GitHubCondition:
        """The repository name (e.g., 'owner/repo')."""
        return GitHubCondition("github.repository")

    @property
    def actor(self) -> GitHubCondition:
        """The user that triggered the workflow."""
        return GitHubCondition("github.actor")

    @property
    def sha(self) -> GitHubCondition:
        """The commit SHA."""
        return GitHubCondition("github.sha")

    @property
    def head_ref(self) -> GitHubCondition:
        """The head branch for pull requests."""
        return GitHubCondition("github.head_ref")

    @property
    def base_ref(self) -> GitHubCondition:
        """The base branch for pull requests."""
        return GitHubCondition("github.base_ref")


# Global instance for convenient access
github = _GitHubContext()


# =============================================================================
# JobSpec - represents a job in an automation
# =============================================================================


@dataclass
class JobSpec:
    """Specification for a job within an automation.

    Created by recompose.job(). Tracks the task, inputs, dependencies,
    and configuration for generating GHA jobs.

    Attributes:
        job_id: Unique identifier for this job (usually task name)
        task_info: The TaskInfo for the task this job runs
        inputs: Input values/references to pass to the task
        needs: Explicit job dependencies (inferred deps added automatically)
        runs_on: Runner specification (default: "ubuntu-latest")
        matrix: Matrix configuration for parallel jobs
        condition: Condition expression for job-level if:

    """

    job_id: str
    """Unique identifier for this job."""

    task_info: TaskInfo
    """The task this job runs."""

    inputs: dict[str, Any] = field(default_factory=dict)
    """Input values/references for the task."""

    needs: list[JobSpec] = field(default_factory=list)
    """Explicit dependencies (inferred deps added in get_all_dependencies)."""

    runs_on: str = "ubuntu-latest"
    """Runner specification."""

    matrix: dict[str, list[Any]] | None = None
    """Matrix configuration for parallel jobs."""

    condition: ConditionExpr | None = None
    """Condition for job-level if:."""

    _inferred_deps: list[JobSpec] = field(default_factory=list, repr=False)
    """Dependencies inferred from input references."""

    def get(self, output_name: str) -> JobOutputRef:
        """Get a reference to an output of this job.

        Args:
            output_name: Name of the output (must be declared in task's outputs)

        Returns:
            JobOutputRef that can be used in other jobs' inputs

        Raises:
            ValueError: If output_name not in task's declared outputs

        """
        if output_name not in self.task_info.outputs:
            available = ", ".join(self.task_info.outputs) or "(none)"
            raise ValueError(
                f"Task '{self.task_info.name}' has no output '{output_name}'. Declared outputs: {available}"
            )
        return JobOutputRef(self.job_id, output_name)

    def artifact(self, artifact_name: str) -> ArtifactRef:
        """Get a reference to an artifact of this job.

        Args:
            artifact_name: Name of the artifact (must be declared in task's artifacts)

        Returns:
            ArtifactRef that can be used in other jobs' inputs

        Raises:
            ValueError: If artifact_name not in task's declared artifacts

        """
        if artifact_name not in self.task_info.artifacts:
            available = ", ".join(self.task_info.artifacts) or "(none)"
            raise ValueError(
                f"Task '{self.task_info.name}' has no artifact '{artifact_name}'. Declared artifacts: {available}"
            )
        return ArtifactRef(self.job_id, artifact_name)

    def get_all_dependencies(self) -> list[JobSpec]:
        """Get all dependencies (explicit + inferred)."""
        # Combine explicit and inferred, removing duplicates while preserving order
        seen = set()
        all_deps = []
        for dep in self.needs + self._inferred_deps:
            if dep.job_id not in seen:
                seen.add(dep.job_id)
                all_deps.append(dep)
        return all_deps

    def __repr__(self) -> str:
        return f"JobSpec({self.job_id})"


# =============================================================================
# Automation Context
# =============================================================================


# Registry mapping job_id -> JobSpec (for reference resolution)
_job_registry: dict[str, JobSpec] = {}


def _get_job_by_id(job_id: str) -> JobSpec | None:
    """Get a job from the current automation context by ID."""
    return _job_registry.get(job_id)


def _clear_job_registry() -> None:
    """Clear the job registry (called at start/end of automation)."""
    _job_registry.clear()


def _register_job(job_spec: JobSpec) -> None:
    """Register a job in the current automation context."""
    _job_registry[job_spec.job_id] = job_spec


# =============================================================================
# job() function - creates JobSpec
# =============================================================================


def job(
    task: TaskWrapper[..., Any],
    *,
    inputs: dict[str, Any] | None = None,
    needs: list[JobSpec] | None = None,
    runs_on: str = "ubuntu-latest",
    matrix: dict[str, list[Any]] | None = None,
    condition: ConditionExpr | None = None,
    job_id: str | None = None,
) -> JobSpec:
    """Create a job specification for an automation.

    This function can only be called inside an @automation-decorated function.
    It creates a JobSpec that will be rendered as a GHA job.

    Args:
        task: The task to run (must be @task decorated)
        inputs: Input values for the task (can include refs to other jobs)
        needs: Explicit dependencies on other jobs
        runs_on: Runner specification (default: "ubuntu-latest")
        matrix: Matrix configuration for parallel execution
        condition: Condition expression for job-level if:
        job_id: Custom job ID (default: task name)

    Returns:
        JobSpec that can be used to reference outputs/artifacts

    Raises:
        RuntimeError: If called outside an @automation function
        TypeError: If task is not a @task-decorated function

    Example:
        @automation
        def ci() -> None:
            build_job = job(build_wheel)
            test_job = job(
                test_wheel,
                inputs={"wheel": build_job.get("wheel_path")},
            )

    """
    from .context import get_automation_context

    ctx = get_automation_context()
    if ctx is None:
        raise RuntimeError("job() can only be called inside an @automation-decorated function.")

    # Validate task
    task_info = getattr(task, "_task_info", None)
    if task_info is None:
        raise TypeError(f"job() requires a @task-decorated function, got {type(task).__name__}")

    # Generate job_id from task name if not provided
    actual_job_id = job_id or task_info.name

    # Check for duplicate job_id
    if actual_job_id in _job_registry:
        raise ValueError(
            f"Duplicate job_id '{actual_job_id}'. Each job must have a unique ID. "
            f"Use job_id='...' to specify a custom ID."
        )

    # Create JobSpec
    job_spec = JobSpec(
        job_id=actual_job_id,
        task_info=task_info,
        inputs=inputs or {},
        needs=needs or [],
        runs_on=runs_on,
        matrix=matrix,
        condition=condition,
    )

    # Infer dependencies from input references
    _infer_dependencies(job_spec)

    # Register and add to context
    _register_job(job_spec)
    ctx.add_job(job_spec)

    return job_spec


def _infer_dependencies(job_spec: JobSpec) -> None:
    """Infer dependencies from JobOutputRef and ArtifactRef in inputs."""
    inferred: list[JobSpec] = []

    for value in job_spec.inputs.values():
        if isinstance(value, JobOutputRef):
            dep_job = _get_job_by_id(value.job_id)
            if dep_job is None:
                raise ValueError(
                    f"JobOutputRef references unknown job '{value.job_id}'. "
                    f"Make sure the job is created before referencing its outputs."
                )
            if dep_job not in inferred:
                inferred.append(dep_job)

        elif isinstance(value, ArtifactRef):
            dep_job = _get_job_by_id(value.job_id)
            if dep_job is None:
                raise ValueError(
                    f"ArtifactRef references unknown job '{value.job_id}'. "
                    f"Make sure the job is created before referencing its artifacts."
                )
            if dep_job not in inferred:
                inferred.append(dep_job)

    job_spec._inferred_deps = inferred


# =============================================================================
# Trigger Types (stubs for Phase 3)
# =============================================================================


@dataclass
class Trigger:
    """Base class for workflow triggers."""

    def __or__(self, other: Trigger) -> CombinedTrigger:
        """Combine triggers with OR."""
        return CombinedTrigger([self, other])

    def to_gha_dict(self) -> dict[str, Any]:
        """Convert to GHA 'on:' dict format."""
        raise NotImplementedError


@dataclass
class CombinedTrigger(Trigger):
    """Multiple triggers combined with OR."""

    triggers: list[Trigger]

    def __or__(self, other: Trigger) -> CombinedTrigger:
        return CombinedTrigger(self.triggers + [other])

    def to_gha_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for trigger in self.triggers:
            result.update(trigger.to_gha_dict())
        return result


@dataclass
class PushTrigger(Trigger):
    """Trigger on push events."""

    branches: list[str] | None = None
    tags: list[str] | None = None
    paths: list[str] | None = None

    def to_gha_dict(self) -> dict[str, Any]:
        config: dict[str, Any] = {}
        if self.branches:
            config["branches"] = self.branches
        if self.tags:
            config["tags"] = self.tags
        if self.paths:
            config["paths"] = self.paths
        return {"push": config or None}


@dataclass
class PullRequestTrigger(Trigger):
    """Trigger on pull request events."""

    branches: list[str] | None = None
    types: list[str] | None = None
    paths: list[str] | None = None

    def to_gha_dict(self) -> dict[str, Any]:
        config: dict[str, Any] = {}
        if self.branches:
            config["branches"] = self.branches
        if self.types:
            config["types"] = self.types
        if self.paths:
            config["paths"] = self.paths
        return {"pull_request": config or None}


@dataclass
class ScheduleTrigger(Trigger):
    """Trigger on schedule."""

    cron: str

    def to_gha_dict(self) -> dict[str, Any]:
        return {"schedule": [{"cron": self.cron}]}


@dataclass
class WorkflowDispatchTrigger(Trigger):
    """Trigger on manual workflow dispatch."""

    # Inputs are populated from automation's InputParam parameters
    inputs: dict[str, dict[str, Any]] = field(default_factory=dict)

    def to_gha_dict(self) -> dict[str, Any]:
        if self.inputs:
            return {"workflow_dispatch": {"inputs": self.inputs}}
        return {"workflow_dispatch": None}


# Convenience functions for creating triggers
def on_push(
    branches: list[str] | None = None,
    tags: list[str] | None = None,
    paths: list[str] | None = None,
) -> PushTrigger:
    """Create a push trigger."""
    return PushTrigger(branches=branches, tags=tags, paths=paths)


def on_pull_request(
    branches: list[str] | None = None,
    types: list[str] | None = None,
    paths: list[str] | None = None,
) -> PullRequestTrigger:
    """Create a pull request trigger."""
    return PullRequestTrigger(branches=branches, types=types, paths=paths)


def on_schedule(cron: str) -> ScheduleTrigger:
    """Create a schedule trigger."""
    return ScheduleTrigger(cron=cron)


def on_workflow_dispatch() -> WorkflowDispatchTrigger:
    """Create a workflow dispatch trigger."""
    return WorkflowDispatchTrigger()


# =============================================================================
# AutomationInfo and @automation decorator
# =============================================================================


@dataclass
class AutomationInfo:
    """Metadata about a registered automation."""

    name: str
    """Short name of the automation."""

    module: str
    """Module where the automation is defined."""

    fn: Callable[..., None]
    """The wrapped function."""

    original_fn: Callable[..., None]
    """The original unwrapped function."""

    signature: inspect.Signature
    """Function signature."""

    doc: str | None
    """Docstring."""

    trigger: Trigger | None = None
    """Workflow trigger configuration."""

    input_params: dict[str, InputParam[Any]] = field(default_factory=dict)
    """InputParam objects from the signature."""

    wrapper: Any = None
    """Reference to the AutomationWrapper (set after wrapper creation)."""

    @property
    def full_name(self) -> str:
        """Full qualified name of the automation."""
        return f"{self.module}:{self.name}"


class AutomationWrapper:
    """Wrapper for @automation-decorated functions."""

    def __init__(
        self,
        info: AutomationInfo,
        original_fn: Callable[..., None],
    ):
        self._automation_info = info
        self._original_fn = original_fn
        functools.update_wrapper(self, original_fn)

    def __call__(self, **kwargs: Any) -> list[JobSpec]:
        """Execute the automation and return the list of jobs."""
        from .context import AutomationContext, set_automation_context

        # Create automation context
        ctx = AutomationContext(
            automation_name=self._automation_info.name,
            input_params=kwargs,
        )

        # Clear and set up job registry
        _clear_job_registry()
        set_automation_context(ctx)

        try:
            # Set up InputParam values from kwargs
            for param_name, param_obj in self._automation_info.input_params.items():
                if param_name in kwargs:
                    param_obj._set_value(kwargs[param_name])

            # Execute the automation function
            self._original_fn(**kwargs)

            return ctx.jobs
        finally:
            set_automation_context(None)
            _clear_job_registry()

    def plan(self, **kwargs: Any) -> list[JobSpec]:
        """Build the automation plan without side effects (alias for __call__)."""
        return self(**kwargs)

    @property
    def info(self) -> AutomationInfo:
        """Get the automation info."""
        return self._automation_info


@overload
def automation(fn: Callable[..., None]) -> AutomationWrapper: ...


@overload
def automation(
    *,
    trigger: Trigger | None = None,
) -> Callable[[Callable[..., None]], AutomationWrapper]: ...


def automation(
    fn: Callable[..., None] | None = None,
    *,
    trigger: Trigger | None = None,
) -> AutomationWrapper | Callable[[Callable[..., None]], AutomationWrapper]:
    """
    Decorator to mark a function as a recompose automation.

    Automations orchestrate multiple tasks as jobs in a GHA workflow.
    Inside an automation, use recompose.job() to define jobs.

    Args:
        trigger: Workflow trigger configuration (on_push, on_pull_request, etc.)

    Example:
        @recompose.automation(trigger=on_push(branches=["main"]))
        def ci() -> None:
            '''CI pipeline.'''
            lint_job = recompose.job(lint)
            build_job = recompose.job(build_wheel)
            test_job = recompose.job(
                test_wheel,
                inputs={"wheel": build_job.get("wheel_path")},
            )

    InputParam parameters become workflow_dispatch inputs:

        @recompose.automation
        def deploy(
            environment: InputParam[str],
            skip_tests: InputParam[bool] = InputParam(default=False),
        ) -> None:
            test_job = recompose.job(test, condition=~skip_tests)
            deploy_job = recompose.job(
                deploy_task,
                inputs={"env": environment},
            )

    """

    def decorator(func: Callable[..., None]) -> AutomationWrapper:
        sig = inspect.signature(func)

        # Extract InputParam parameters from signature
        input_params: dict[str, InputParam[Any]] = {}
        for param_name, param in sig.parameters.items():
            # Check if default is an InputParam
            if isinstance(param.default, InputParam):
                param.default._set_name(param_name)
                input_params[param_name] = param.default
            # Check if annotation indicates InputParam (no default = required)
            elif (
                param.annotation is not inspect.Parameter.empty
                and hasattr(param.annotation, "__origin__")
                and getattr(param.annotation, "__origin__", None) is InputParam
            ):
                # Create InputParam for required parameter
                ip = InputParam[Any](required=True)
                ip._set_name(param_name)
                input_params[param_name] = ip

        # Create AutomationInfo
        info = AutomationInfo(
            name=func.__name__,
            module=func.__module__,
            fn=func,  # Will be replaced with wrapper
            original_fn=func,
            signature=sig,
            doc=func.__doc__,
            trigger=trigger,
            input_params=input_params,
        )

        # Create wrapper
        wrapper = AutomationWrapper(info, func)
        info.fn = wrapper  # Update to point to wrapper
        info.wrapper = wrapper  # Store reference for GHA generation

        return wrapper

    # Handle both @automation and @automation(...) forms
    if fn is not None:
        return decorator(fn)
    return decorator


# =============================================================================
# Dispatchable Tasks (P14 Phase 5)
# =============================================================================


@dataclass
class DispatchInput:
    """Base class for workflow_dispatch input specifications.

    These are used with make_dispatchable() to define workflow inputs.
    """

    description: str | None = None
    required: bool = False

    def to_gha_dict(self) -> dict[str, Any]:
        """Convert to GHA workflow_dispatch input format."""
        raise NotImplementedError


@dataclass
class StringInput(DispatchInput):
    """String input for workflow_dispatch.

    Example:
        workflow = make_dispatchable(
            my_task,
            inputs={"name": StringInput(default="world", description="Name to greet")},
        )

    """

    default: str | None = None

    def to_gha_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "type": "string",
            "required": self.required,
        }
        if self.description:
            d["description"] = self.description
        else:
            d["description"] = ""
        if self.default is not None:
            d["default"] = self.default
        return d


@dataclass
class BoolInput(DispatchInput):
    """Boolean input for workflow_dispatch.

    Example:
        workflow = make_dispatchable(
            my_task,
            inputs={"verbose": BoolInput(default=False, description="Enable verbose output")},
        )

    """

    default: bool = False

    def to_gha_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "type": "boolean",
            "required": self.required,
        }
        if self.description:
            d["description"] = self.description
        else:
            d["description"] = ""
        d["default"] = self.default
        return d


@dataclass
class ChoiceInput(DispatchInput):
    """Choice input for workflow_dispatch.

    Example:
        workflow = make_dispatchable(
            deploy_task,
            inputs={"environment": ChoiceInput(
                choices=["dev", "staging", "prod"],
                default="staging",
                description="Target environment",
            )},
        )

    """

    choices: list[str] = field(default_factory=list)
    default: str | None = None

    def to_gha_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "type": "choice",
            "required": self.required,
            "options": self.choices,
        }
        if self.description:
            d["description"] = self.description
        else:
            d["description"] = ""
        if self.default is not None:
            d["default"] = self.default
        return d


@dataclass
class DispatchableInfo:
    """Metadata about a dispatchable task."""

    name: str
    """Workflow name (defaults to task name)."""

    task_info: TaskInfo
    """The underlying task."""

    inputs: dict[str, DispatchInput]
    """Workflow dispatch inputs."""

    @property
    def full_name(self) -> str:
        """Full qualified name."""
        return f"{self.task_info.module}:{self.name}"


class Dispatchable:
    """A task wrapped for workflow_dispatch triggering.

    Created by make_dispatchable(). Can be rendered to a single-job
    workflow with workflow_dispatch trigger.

    Example:
        lint_workflow = make_dispatchable(lint)
        test_workflow = make_dispatchable(
            test,
            inputs={"verbose": BoolInput(default=False)},
        )

        # Add to App for workflow generation
        app = App(
            commands=[...],
            dispatchables=[lint_workflow, test_workflow],
        )

    """

    def __init__(self, info: DispatchableInfo):
        self._info = info

    @property
    def info(self) -> DispatchableInfo:
        """Get the dispatchable info."""
        return self._info

    @property
    def name(self) -> str:
        """Get the workflow name."""
        return self._info.name

    @property
    def task_info(self) -> TaskInfo:
        """Get the underlying task info."""
        return self._info.task_info

    def __repr__(self) -> str:
        return f"Dispatchable({self.name})"


def _infer_inputs_from_task(task_info: TaskInfo) -> dict[str, DispatchInput]:
    """Infer workflow_dispatch inputs from task signature."""
    inputs: dict[str, DispatchInput] = {}

    for param_name, param in task_info.signature.parameters.items():
        annotation = param.annotation
        has_default = param.default is not inspect.Parameter.empty
        default_value = param.default if has_default else None

        # Determine input type from annotation
        if annotation is bool or isinstance(default_value, bool):
            inputs[param_name] = BoolInput(
                default=default_value if isinstance(default_value, bool) else False,
                required=not has_default,
                description=f"Parameter: {param_name}",
            )
        elif annotation is int or annotation is float:
            # GHA doesn't have a native number input, use string
            inputs[param_name] = StringInput(
                default=str(default_value) if default_value is not None else None,
                required=not has_default,
                description=f"Parameter: {param_name}",
            )
        else:
            # Default to string
            str_default = None
            if default_value is not None:
                if isinstance(default_value, Path):
                    str_default = str(default_value)
                elif isinstance(default_value, str):
                    str_default = default_value
                else:
                    str_default = str(default_value)
            inputs[param_name] = StringInput(
                default=str_default,
                required=not has_default,
                description=f"Parameter: {param_name}",
            )

    return inputs


def make_dispatchable(
    task: TaskWrapper[..., Any],
    *,
    inputs: dict[str, DispatchInput] | None = None,
    name: str | None = None,
) -> AutomationWrapper:
    """Create an automation with workflow_dispatch trigger for a single task.

    This is a convenience function for creating a simple automation that:
    - Has workflow_dispatch trigger (manually triggerable in GHA)
    - Runs a single task
    - Exposes task parameters as workflow inputs

    Args:
        task: The task to make dispatchable (must be @task decorated)
        inputs: Optional workflow inputs. If None, infers from task signature.
                If provided, these override/extend the inferred inputs.
        name: Optional workflow name. Defaults to task name.

    Returns:
        AutomationWrapper that can be added to App.automations.

    Example:
        # Simple - infer inputs from task
        lint_workflow = make_dispatchable(lint)

        # With explicit inputs
        test_workflow = make_dispatchable(
            test,
            inputs={
                "verbose": BoolInput(default=False, description="Verbose output"),
            },
        )

        # With custom name
        deploy_prod = make_dispatchable(deploy, name="deploy_production")

        # Register in App
        app = App(
            automations=[lint_workflow, test_workflow, deploy_prod],
            ...
        )

    """
    # Validate task
    task_info = getattr(task, "_task_info", None)
    if task_info is None:
        raise TypeError(f"make_dispatchable() requires a @task-decorated function, got {type(task).__name__}")

    # Determine inputs
    if inputs is None:
        # Infer from task signature
        dispatch_inputs = _infer_inputs_from_task(task_info)
    else:
        dispatch_inputs = inputs

    # Determine name
    workflow_name = name if name is not None else task_info.name

    # Convert DispatchInput to InputParam
    input_params: dict[str, InputParam[Any]] = {}
    for param_name, dispatch_input in dispatch_inputs.items():
        # Determine type and create InputParam
        if isinstance(dispatch_input, BoolInput):
            ip: InputParam[Any] = InputParam(
                default=dispatch_input.default,
                description=dispatch_input.description,
                required=dispatch_input.required,
            )
        elif isinstance(dispatch_input, ChoiceInput):
            ip = InputParam(
                default=dispatch_input.default,
                description=dispatch_input.description,
                required=dispatch_input.required,
                choices=dispatch_input.choices,
            )
        elif isinstance(dispatch_input, StringInput):
            ip = InputParam(
                default=dispatch_input.default,
                description=dispatch_input.description,
                required=dispatch_input.required,
            )
        else:
            # Fallback for unknown DispatchInput subclasses
            ip = InputParam(
                default=None,
                description=dispatch_input.description,
                required=dispatch_input.required,
            )
        ip._set_name(param_name)
        input_params[param_name] = ip

    # Create the automation function that creates a single job
    def automation_fn(**_kwargs: Any) -> None:
        # Pass InputParam refs as job inputs
        job_inputs = {pname: param.to_ref() for pname, param in input_params.items()}
        job(task, inputs=job_inputs)

    # Create the AutomationInfo
    info = AutomationInfo(
        name=workflow_name,
        module=task_info.module,
        fn=automation_fn,  # Will be replaced with wrapper
        original_fn=automation_fn,
        signature=inspect.signature(automation_fn),
        doc=task_info.doc,
        trigger=on_workflow_dispatch(),
        input_params=input_params,
    )

    # Create wrapper
    wrapper = AutomationWrapper(info, automation_fn)
    info.fn = wrapper
    info.wrapper = wrapper

    return wrapper
