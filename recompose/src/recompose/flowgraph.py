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

    Tracks all TaskNodes created during flow construction and provides
    utilities for:
    - Topological sorting (valid execution order)
    - Finding parallelizable groups
    - Visualizing the graph
    """

    nodes: list[TaskNode[Any]] = field(default_factory=list)
    terminal: TaskNode[Any] | None = None

    def add_node(self, node: TaskNode[Any]) -> None:
        """Register a node in the plan."""
        self.nodes.append(node)

    def inject_setup_node(self, task_info: TaskInfo) -> TaskNode[None] | None:
        """
        Inject a setup_workspace node into the plan.

        The setup node is inserted after all GHA action nodes but before the
        first non-GHA task node. It depends on all GHA actions and all non-GHA
        tasks depend on it.

        Args:
            task_info: TaskInfo for the setup_workspace virtual task.

        Returns:
            The injected TaskNode, or None if there are no non-GHA tasks.

        """
        # Separate GHA actions from regular tasks
        gha_nodes = [n for n in self.nodes if n.task_info.is_gha_action]
        task_nodes = [n for n in self.nodes if not n.task_info.is_gha_action]

        if not task_nodes:
            # No regular tasks, no setup needed
            return None

        # Create the setup node - it depends on all GHA actions
        setup_node: TaskNode[None] = TaskNode(
            task_info=task_info,
            kwargs={},
        )

        # Make setup node depend on all GHA actions (not as kwargs, but we need
        # to ensure ordering). We do this by making the first task node's
        # original dependencies now depend on setup, and setup depends on GHA.
        # Actually, simpler: we'll rewrite the node list with setup in the right place.

        # Insert setup node between GHA actions and tasks
        # The topological sort will respect the list order for nodes at the same level
        new_nodes = gha_nodes + [setup_node] + task_nodes
        self.nodes = new_nodes

        return setup_node

    def inject_condition_checks(self, condition_task_info: TaskInfo) -> list[TaskNode[bool]]:
        """
        Inject condition-check nodes for conditional tasks.

        For each unique condition expression, creates a condition-check node
        that evaluates it. Conditional tasks are updated with a reference to
        their condition-check step.

        Args:
            condition_task_info: TaskInfo for the eval_condition pseudo-task.

        Returns:
            List of injected condition-check TaskNodes.

        """
        # Find all unique conditions (by serialized form)
        condition_map: dict[str, tuple[Expr, list[TaskNode[Any]]]] = {}
        for node in self.nodes:
            if node.condition is not None:
                # Use serialized form as key for deduplication
                key = str(node.condition.serialize())
                if key not in condition_map:
                    condition_map[key] = (node.condition, [])
                condition_map[key][1].append(node)

        if not condition_map:
            return []

        # Create condition-check nodes and inject them
        check_nodes: list[TaskNode[bool]] = []
        new_nodes: list[TaskNode[Any]] = []

        # Process nodes in original order, injecting checks before first conditional
        injected_conditions: set[str] = set()

        for node in self.nodes:
            if node.condition is not None:
                key = str(node.condition.serialize())
                if key not in injected_conditions:
                    # Create and inject the condition-check node
                    condition_expr, _ = condition_map[key]
                    check_node: TaskNode[bool] = TaskNode(
                        task_info=condition_task_info,
                        kwargs={"condition_data": condition_expr.serialize()},
                    )
                    new_nodes.append(check_node)
                    check_nodes.append(check_node)
                    injected_conditions.add(key)

            new_nodes.append(node)

        self.nodes = new_nodes

        # Now assign step names so we can set condition_check_step references
        self.assign_step_names()

        # Update conditional nodes with their condition-check step name
        for key, (_, conditional_nodes) in condition_map.items():
            # Find the check node for this condition
            for check_node in check_nodes:
                if str(check_node.kwargs.get("condition_data")) == key.replace("'", '"'):
                    # This is a bit fragile - let's use a better approach
                    pass

        # Better: match by position - check nodes are in same order as condition_map
        check_iter = iter(check_nodes)
        for key, (_, conditional_nodes) in condition_map.items():
            check_node = next(check_iter)
            for node in conditional_nodes:
                node.condition_check_step = check_node.step_name

        return check_nodes

    def get_execution_order(self) -> list[TaskNode[Any]]:
        """
        Return nodes in topological order (dependencies before dependents).

        Uses Kahn's algorithm for topological sorting.
        """
        if not self.nodes:
            return []

        # Build adjacency list and in-degree count
        in_degree: dict[str, int] = {n.node_id: 0 for n in self.nodes}
        dependents: dict[str, list[TaskNode[Any]]] = {n.node_id: [] for n in self.nodes}
        node_by_id: dict[str, TaskNode[Any]] = {n.node_id: n for n in self.nodes}

        for node in self.nodes:
            for dep in node.dependencies:
                if dep.node_id in dependents:
                    dependents[dep.node_id].append(node)
                    in_degree[node.node_id] += 1

        # Start with nodes that have no dependencies
        queue = [node_by_id[nid] for nid, deg in in_degree.items() if deg == 0]
        result: list[TaskNode[Any]] = []

        while queue:
            # Take first node (FIFO for deterministic order)
            node = queue.pop(0)
            result.append(node)

            # Reduce in-degree for dependents
            for dependent in dependents[node.node_id]:
                in_degree[dependent.node_id] -= 1
                if in_degree[dependent.node_id] == 0:
                    queue.append(dependent)

        # Check for cycles
        if len(result) != len(self.nodes):
            raise ValueError("Cycle detected in flow graph")

        return result

    def get_parallelizable_groups(self) -> list[list[TaskNode[Any]]]:
        """
        Group nodes by levels - nodes in the same level can run in parallel.

        Returns a list of groups, where each group contains nodes that have
        no dependencies on each other and can be executed concurrently.
        """
        if not self.nodes:
            return []

        # Build dependency info
        node_by_id: dict[str, TaskNode[Any]] = {n.node_id: n for n in self.nodes}
        level: dict[str, int] = {}

        def get_level(node: TaskNode[Any]) -> int:
            if node.node_id in level:
                return level[node.node_id]

            if not node.dependencies:
                level[node.node_id] = 0
            else:
                dep_levels = [get_level(d) for d in node.dependencies if d.node_id in node_by_id]
                level[node.node_id] = (max(dep_levels) + 1) if dep_levels else 0

            return level[node.node_id]

        # Compute levels for all nodes
        for node in self.nodes:
            get_level(node)

        # Group by level
        max_level = max(level.values()) if level else 0
        groups: list[list[TaskNode[Any]]] = [[] for _ in range(max_level + 1)]
        for node in self.nodes:
            groups[level[node.node_id]].append(node)

        return groups

    def assign_step_names(self) -> None:
        """
        Assign sequential step names to all nodes based on execution order.

        Step names have the format "NN_task_name" where NN is a zero-padded
        sequence number (e.g., "01_fetch_source", "02_compile_source").

        This makes execution order explicit and ensures unique names even
        when the same task is used multiple times in a flow.
        """
        execution_order = self.get_execution_order()
        num_digits = len(str(len(execution_order)))  # Enough digits to fit all steps

        for i, node in enumerate(execution_order, start=1):
            node.step_name = f"{i:0{num_digits}d}_{node.task_info.name}"

    def get_step(self, step_ref: str) -> TaskNode[Any] | None:
        """
        Find a step by name, number, or task name.

        Args:
            step_ref: Can be:
                - Full step name: "03_run_unit_tests"
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
                    # Extract number from "NN_task_name"
                    num_part = node.step_name.split("_")[0]
                    if int(num_part) == step_num:
                        return node
        except ValueError:
            pass

        # Try matching by task name (if unambiguous)
        matches = [n for n in self.nodes if n.task_info.name == step_ref]
        if len(matches) == 1:
            return matches[0]

        return None

    def get_steps(self) -> list[tuple[str, TaskNode[Any]]]:
        """
        Return all steps in execution order with their step names.

        Returns:
            List of (step_name, node) tuples.

        """
        if self.nodes and self.nodes[0].step_name is None:
            self.assign_step_names()

        return [(n.step_name or n.name, n) for n in self.get_execution_order()]

    def visualize(self) -> str:
        """Return an ASCII representation of the flow graph."""
        if not self.nodes:
            return "(empty flow)"

        # Ensure step names are assigned
        if self.nodes[0].step_name is None:
            self.assign_step_names()

        lines: list[str] = []
        groups = self.get_parallelizable_groups()

        for i, group in enumerate(groups):
            level_str = f"Level {i}: "
            node_strs = []
            for node in group:
                display_name = node.step_name or node.name
                deps = [d.step_name or d.name for d in node.dependencies]
                if deps:
                    node_strs.append(f"{display_name} <- [{', '.join(deps)}]")
                else:
                    node_strs.append(display_name)
            lines.append(level_str + " | ".join(node_strs))

        if self.terminal:
            terminal_name = self.terminal.step_name or self.terminal.name
            lines.append(f"Terminal: {terminal_name}")

        return "\n".join(lines)


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
