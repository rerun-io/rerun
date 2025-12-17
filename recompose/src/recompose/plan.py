"""Declarative flow graph types for recompose.

This module provides the types needed for declarative flow execution:
- Input[T]: Type alias for flow inputs (literal values or task outputs)
- TaskNode[T]: Represents a deferred task execution in a flow graph
- FlowPlan: The execution graph for a flow
- InputPlaceholder[T]: Placeholder for flow inputs during plan construction
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Generic, TypeVar

from .expr import BinaryExpr, Expr, InputExpr, LiteralExpr, UnaryExpr

if TYPE_CHECKING:
    from .task import TaskInfo

T = TypeVar("T")


@dataclass
class InputPlaceholder(Generic[T]):
    """
    Placeholder for a flow input parameter during plan construction.

    When building a FlowPlan for GHA generation, we don't have actual values
    for required flow parameters. InputPlaceholder stands in for these values,
    allowing the flow function body to execute and build the task graph.

    When the placeholder is passed to a task call, it's stored in
    the TaskNode kwargs. Later, when generating GHA YAML, we recognize these
    placeholders and emit references like `${{ inputs.name }}`.

    Example:
        # During GHA generation for a flow with required 'repo' parameter:
        placeholder = InputPlaceholder[str](name="repo")

        # The flow body receives this placeholder:
        @flow
        def build_flow(*, repo: str) -> None:
            clone(repo=repo)  # repo is actually an InputPlaceholder

        # The placeholder is stored in the TaskNode kwargs and later
        # serialized to "${{ inputs.repo }}" in the GHA workflow YAML.

    """

    name: str
    """The name of the flow parameter this placeholder represents."""

    annotation: type[T] | None = None
    """The type annotation of the parameter (for documentation/debugging)."""

    default: T | None = None
    """The default value, if any (used for optional params)."""

    def value(self) -> T:
        """
        Get the placeholder's value for passing to tasks.

        Type signature says T, but at runtime returns self (the InputPlaceholder).
        This enables type-safe flow composition with placeholders:

            @flow
            def my_flow(*, name: str) -> None:
                # name is InputPlaceholder[str] at runtime during GHA generation
                greet(name=name.value())
        """
        return self  # type: ignore[return-value]

    @property
    def ok(self) -> bool:
        """Mimic Result.ok for type compatibility."""
        return True

    @property
    def failed(self) -> bool:
        """Mimic Result.failed for type compatibility."""
        return False

    @property
    def error(self) -> str | None:
        """Mimic Result.error for type compatibility."""
        return None

    def __repr__(self) -> str:
        type_str = self.annotation.__name__ if self.annotation else "Any"
        return f"InputPlaceholder({self.name}: {type_str})"

    def __str__(self) -> str:
        # Return a string representation that looks like the GHA reference
        # This is useful for debugging and makes errors more understandable
        return f"${{{{ inputs.{self.name} }}}}"

    def __bool__(self) -> bool:
        """Raise error when flow parameter is used in Python control flow."""
        raise TypeError(
            f"Flow parameter '{self.name}' cannot be used directly in Python control flow "
            f"(e.g., 'if {self.name}:').\n\n"
            f"For conditional execution, use 'with recompose.run_if({self.name}):' instead.\n"
            f"This creates a conditional block that works both locally and in GitHub Actions."
        )

    def to_expr(self) -> InputExpr:
        """Convert to an expression for use with run_if()."""
        return InputExpr(self.name)

    def __eq__(self, other: object) -> BinaryExpr:  # type: ignore[override]
        """Create equality comparison expression for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(self.to_expr(), "==", other_expr)

    def __ne__(self, other: object) -> BinaryExpr:  # type: ignore[override]
        """Create inequality comparison expression for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(self.to_expr(), "!=", other_expr)

    def __and__(self, other: Expr | bool) -> BinaryExpr:
        """Create logical AND expression for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(self.to_expr(), "and", other_expr)

    def __rand__(self, other: Expr | bool) -> BinaryExpr:
        """Create logical AND expression (reversed) for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(other_expr, "and", self.to_expr())

    def __or__(self, other: Expr | bool) -> BinaryExpr:
        """Create logical OR expression for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(self.to_expr(), "or", other_expr)

    def __ror__(self, other: Expr | bool) -> BinaryExpr:
        """Create logical OR expression (reversed) for use with run_if()."""
        other_expr = LiteralExpr(other) if not isinstance(other, Expr) else other
        return BinaryExpr(other_expr, "or", self.to_expr())

    def __invert__(self) -> UnaryExpr:
        """Create logical NOT expression for use with run_if()."""
        return UnaryExpr("not", self.to_expr())


@dataclass
class TaskNode(Generic[T]):
    """
    Represents a deferred task execution in a flow graph (a "step").

    When you call `task(arg=value)` inside a flow, it returns a TaskNode
    that mimics Result[T] for type-checking purposes. The TaskNode captures:
    - What task to run
    - What arguments to pass (which may include other TaskNodes as dependencies)
    - A unique ID for tracking
    - A step_name assigned by the FlowPlan (e.g., "01_fetch_source")

    The generic parameter T represents the value type that the task will
    produce when executed.

    Usage pattern in flows:
        @flow
        def build_flow():
            # direct call returns Result[Path] to type checker, TaskNode[Path] at runtime
            compiled = compile(source=Path("src/"))

            # .value returns Path to type checker, but TaskNode[Path] at runtime
            # This TaskNode is recognized as a dependency by the next call
            tested = test(binary=compiled.value)

            return tested

    The .value property enables type-safe flow composition:
    - Type checker sees: compile() -> Result[Path], .value -> Path
    - Runtime behavior: compile() -> TaskNode[Path], .value -> TaskNode[Path]
    - The receiving direct call validates that inputs are literals or TaskNode/InputPlaceholder
    """

    task_info: TaskInfo
    kwargs: dict[str, Any] = field(default_factory=dict)
    node_id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    step_name: str | None = field(default=None)  # Assigned by FlowPlan.assign_step_names()
    condition: Expr | None = field(default=None)  # Condition for conditional execution (run_if)
    condition_check_step: str | None = field(default=None)  # Step name of the condition-check this depends on

    def value(self) -> T:
        """
        Get the task's output value for passing to other tasks.

        Type signature says T, but at runtime returns self (the TaskNode).
        This enables type-safe flow composition:

            result = greet(name="World")  # Type: Result[str]
            echo(message=result.value())  # Type: str, Runtime: TaskNode[str]

        The receiving call recognizes TaskNode as a valid Input type.
        """
        return self  # type: ignore[return-value]

    @property
    def ok(self) -> bool:
        """
        Mimic Result.ok for type compatibility.

        In a flow context (plan building), this always returns True since
        we're building the graph, not executing. During actual execution,
        the real Result.ok is used.
        """
        return True

    @property
    def failed(self) -> bool:
        """Mimic Result.failed for type compatibility."""
        return False

    @property
    def error(self) -> str | None:
        """Mimic Result.error for type compatibility."""
        return None

    @property
    def name(self) -> str:
        """Short name of this node (task name)."""
        return self.task_info.name

    @property
    def dependencies(self) -> list[TaskNode[Any]]:
        """Tasks this node depends on (extracted from kwargs)."""
        deps: list[TaskNode[Any]] = []
        for v in self.kwargs.values():
            if isinstance(v, TaskNode):
                deps.append(v)
        return deps

    def __repr__(self) -> str:
        deps_str = ", ".join(d.name for d in self.dependencies) if self.dependencies else "none"
        return f"TaskNode({self.name}, deps=[{deps_str}])"


@dataclass
class FlowPlan:
    """
    The execution graph for a flow.

    Tracks all TaskNodes created during flow construction. Nodes are added
    to the plan in the order they're called during flow function execution.
    Since Python executes sequentially and a TaskNode can only be used
    *after* it's created, `self.nodes` is already in valid execution order
    by construction.

    Condition-check nodes are automatically created when a conditional task
    is added. These are first-class nodes in the plan, not injected later.

    Provides utilities for:
    - Finding parallelizable groups (for visualization)
    - Visualizing the graph
    """

    nodes: list[TaskNode[Any]] = field(default_factory=list)
    terminal: TaskNode[Any] | None = None

    # Track condition-check nodes by serialized condition for deduplication
    _condition_checks: dict[str, TaskNode[bool]] = field(default_factory=dict)
    _condition_counter: int = field(default=0)

    def add_node(self, node: TaskNode[Any]) -> None:
        """
        Register a node in the plan.

        If the node has a condition and no condition-check node exists for it,
        one is automatically created and inserted before this node.
        """
        # If this node has a condition, ensure we have a condition-check node
        if node.condition is not None:
            condition_key = str(node.condition.serialize())

            if condition_key not in self._condition_checks:
                # Create a condition-check node
                check_node = self._create_condition_check_node(node.condition)
                self.nodes.append(check_node)
                self._condition_checks[condition_key] = check_node

            # Link the conditional node to its condition-check step
            check_node = self._condition_checks[condition_key]
            node.condition_check_step = check_node.step_name

        self.nodes.append(node)

    def _create_condition_check_node(self, condition: Expr) -> TaskNode[bool]:
        """Create a condition-check node for the given condition expression."""
        from .task import TaskInfo
        from .result import Ok

        self._condition_counter += 1
        step_name = f"run_if_{self._condition_counter}"

        # Create a TaskInfo for condition evaluation
        def eval_condition_fn(**kwargs: Any) -> Any:
            # This function is executed via --step run_if_N
            # The actual evaluation happens in cli.py
            return Ok(True)

        task_info = TaskInfo(
            name=step_name,
            module="recompose.plan",
            fn=eval_condition_fn,
            original_fn=eval_condition_fn,
            signature=__import__("inspect").Signature(),
            doc=f"Evaluate condition: {condition}",
            is_condition_check=True,
        )

        check_node: TaskNode[bool] = TaskNode(
            task_info=task_info,
            kwargs={"condition_data": condition.serialize()},
        )
        check_node.step_name = step_name  # Pre-assign the step name

        return check_node


    def assign_step_names(self) -> None:
        """
        Assign sequential step names to all nodes based on linear order.

        Step names have the format "step_NN_task_name" where NN is a zero-padded
        sequence number (e.g., "step_01_fetch_source", "step_02_compile_source").

        The "step_" prefix ensures GHA step IDs are valid (must start with
        a letter or underscore, not a digit).

        Nodes that already have step names (e.g., condition check nodes) are
        skipped but still counted in the sequence.

        This makes execution order explicit and ensures unique names even
        when the same task is used multiple times in a flow.
        """
        # Use linear order (self.nodes), not topological sort
        num_digits = len(str(len(self.nodes)))  # Enough digits to fit all steps

        for i, node in enumerate(self.nodes, start=1):
            if node.step_name is None:
                node.step_name = f"step_{i:0{num_digits}d}_{node.task_info.name}"

    def get_step(self, step_ref: str) -> TaskNode[Any] | None:
        """
        Find a step by name, number, or task name.

        Args:
            step_ref: Can be:
                - Full step name: "step_03_run_unit_tests"
                - Just the number: "03" or "3"
                - Task name (if unambiguous): "run_unit_tests"

        Returns:
            The matching TaskNode, or None if not found.

        """
        # Ensure step names are assigned
        if self.nodes and self.nodes[0].step_name is None:
            self.assign_step_names()

        # Try exact match on step_name
        for node in self.nodes:
            if node.step_name == step_ref:
                return node

        # Try matching by number (with or without leading zeros)
        try:
            step_num = int(step_ref)
            for node in self.nodes:
                if node.step_name:
                    # Extract number from "step_NN_task_name"
                    parts = node.step_name.split("_")
                    if len(parts) >= 2 and parts[0] == "step":
                        try:
                            if int(parts[1]) == step_num:
                                return node
                        except ValueError:
                            pass
        except ValueError:
            pass

        # Try matching by task name (if unambiguous)
        matches = [n for n in self.nodes if n.task_info.name == step_ref]
        if len(matches) == 1:
            return matches[0]

        return None

    def get_steps(self) -> list[tuple[str, TaskNode[Any]]]:
        """
        Return all steps in linear order with their step names.

        Uses the order from the flow definition (self.nodes), not topological sort.
        For flows, linear order is already valid by construction.

        Returns:
            List of (step_name, node) tuples.

        """
        if self.nodes and self.nodes[0].step_name is None:
            self.assign_step_names()

        return [(n.step_name or n.name, n) for n in self.nodes]


# =============================================================================
# Input[T] Type Alias
# =============================================================================

# Input[T] represents a value that can be passed to a task call.
# It accepts:
#   - T: A literal value of the expected type
#   - TaskNode[T]: Output from another task call (dependency)
#   - InputPlaceholder[T]: A placeholder for flow parameters (used in GHA generation)
#
# Usage in flow function signatures:
#
#     @recompose.flow
#     def build_pipeline(*, repo: Input[str], debug: Input[bool] = False) -> None:
#         source = clone(repo=repo)  # repo can be str, TaskNode[str], or InputPlaceholder[str]
#         build(source=source, debug=debug)
#
# Note: Python's type system doesn't fully validate the transformation at static
# analysis time (e.g., ensuring TaskNode[str] matches where str is expected).
# Runtime validation is performed in calls.

Input = T | TaskNode[T] | InputPlaceholder[T]  # type: ignore[misc]
"""
Type alias for values accepted by task calls.

Input[T] accepts:
- T: A literal value of the expected type
- TaskNode[T]: Output from another task call
- InputPlaceholder[T]: A placeholder for flow parameters

Example:
    @recompose.flow
    def my_flow(*, name: Input[str]) -> None:
        greet(name=name)  # name can be str, TaskNode[str], or InputPlaceholder[str]
"""
