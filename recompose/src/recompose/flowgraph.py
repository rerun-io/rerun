"""Declarative flow graph types for recompose.

This module provides the types needed for declarative flow execution:
- Input[T]: Type alias for flow inputs (literal values or task outputs)
- TaskNode[T]: Represents a deferred task execution in a flow graph
- FlowPlan: The execution graph for a flow
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Generic, TypeVar

if TYPE_CHECKING:
    from .task import TaskInfo

T = TypeVar("T")


@dataclass
class TaskNode(Generic[T]):
    """
    Represents a deferred task execution in a flow graph (a "step").

    When you call `task.flow(arg=value)` inside a flow, it returns a TaskNode
    instead of executing immediately. The TaskNode captures:
    - What task to run
    - What arguments to pass (which may include other TaskNodes as dependencies)
    - A unique ID for tracking
    - A step_name assigned by the FlowPlan (e.g., "01_fetch_source")

    The generic parameter T represents the type of Result[T] the task will
    produce when executed.

    Example:
        @flow
        def build_flow():
            compiled = compile.flow(source=Path("src/"))  # Returns TaskNode[Path]
            tested = test.flow(binary=compiled)           # compiled is a dependency
            return tested
    """

    task_info: TaskInfo
    kwargs: dict[str, Any] = field(default_factory=dict)
    node_id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    step_name: str | None = field(default=None)  # Assigned by FlowPlan.assign_step_names()

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


# Note: Input[T] is a conceptual type representing T | TaskNode[T]
# In flow function signatures, parameters can accept both literal values
# and TaskNode outputs from other .flow() calls. This is checked at runtime
# rather than compile time since Python's type system doesn't easily support
# this pattern as a generic type alias.
