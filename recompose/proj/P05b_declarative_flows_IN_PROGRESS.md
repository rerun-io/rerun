# P05b: Declarative Flow Execution

**Status:** IN_PROGRESS
**Goal:** Enable flows to build a task graph before execution, enabling dry-run, parallel execution, and subprocess isolation.

## The Problem

Currently flows are imperative - tasks execute immediately when called. This means:
- We only know what ran *after* execution
- Can't preview/dry-run a flow
- Can't parallelize independent tasks
- Can't easily map to external executors (subprocess, GHA)
- Type confusion between planning and execution modes

## The Solution: Explicit .flow() Variant

Tasks have two calling modes with distinct APIs:

1. **Direct call** - `task(arg=value)` → executes immediately, returns `Result[T]`
2. **Flow call** - `task.flow(arg=value)` → returns `TaskNode[T]`, only valid inside flows

```python
@recompose.task
def compile(*, source: Path) -> recompose.Result[Path]:
    # Actual implementation
    return recompose.Ok(output_path)

# Direct execution (normal Python):
result = compile(source=Path("src/"))  # Returns Result[Path]

# Inside a flow (graph building):
@recompose.flow
def build_flow(*, source: Input[Path]):
    compiled = compile.flow(source=source)    # Returns TaskNode[Path]
    tested = run_tests.flow(binary=compiled)  # Takes TaskNode as input
    packaged = package.flow(binary=compiled)  # Also depends on compiled
    return packaged

# Planning - get the graph without executing
plan = build_flow.plan(source=Path("src/"))
# plan.nodes = [compiled, tested, packaged]
# plan shows: compile → [run_tests, package]

# Execution - build graph, then execute
result = build_flow(source=Path("src/"))
```

## Key Types

### Input[T]

Represents a flow input - either a literal value or a task output:

```python
Input[T] = T | TaskNode[T]
```

Used in flow function signatures to accept both:
- Literal values from CLI/caller: `Input[Path]` accepts `Path("src/")`
- Task outputs: `Input[Path]` accepts `compile.flow()` which returns `TaskNode[Path]`

### TaskNode[T]

Represents a deferred task execution in a flow graph:

```python
@dataclass
class TaskNode(Generic[T]):
    """Represents a deferred task execution."""
    task_info: TaskInfo
    kwargs: dict[str, Any]  # May contain other TaskNodes as values
    node_id: str  # Unique identifier

    @property
    def dependencies(self) -> list[TaskNode]:
        """Tasks this depends on (extracted from kwargs)."""
        return [v for v in self.kwargs.values() if isinstance(v, TaskNode)]
```

The generic `T` represents the type of `Result[T]` that the task will produce when executed.

## Key Design: FlowPlan

```python
@dataclass
class FlowPlan:
    """The execution graph for a flow."""
    nodes: list[TaskNode]  # All tasks in execution order
    terminal: TaskNode     # The final task (flow's return value)

    def get_dependencies(self, node: TaskNode) -> list[TaskNode]:
        """Get direct dependencies of a node."""

    def get_execution_order(self) -> list[TaskNode]:
        """Topological sort - tasks in valid execution order."""

    def get_parallelizable_groups(self) -> list[list[TaskNode]]:
        """Group tasks that can run in parallel."""
```

## How .flow() Works

The `@task` decorator adds a `.flow()` method to the wrapped function:

```python
def task(fn):
    @functools.wraps(fn)
    def wrapper(**kwargs) -> Result[T]:
        # Direct execution - runs immediately
        return _execute_task(fn, kwargs)

    def flow_variant(**kwargs) -> TaskNode[T]:
        # Must be inside a flow context
        plan = _current_plan.get()
        if plan is None:
            raise RuntimeError("task.flow() can only be called inside a @flow")

        # Create node and register it
        node = TaskNode(task_info=info, kwargs=kwargs)
        plan.add_node(node)
        return node

    wrapper.flow = flow_variant
    return wrapper
```

This gives us:
- `my_task(arg=val)` → executes immediately, returns `Result[T]`
- `my_task.flow(arg=val)` → builds graph, returns `TaskNode[T]`

**Context tracking for flows:**

```python
_current_plan: ContextVar[FlowPlan | None] = ContextVar("plan", default=None)

@flow
def my_flow():
    # _current_plan is set to a new FlowPlan
    # .flow() calls register nodes in it
    ...
```

## Handling TaskNode in Arguments

When task B depends on task A's output:

```python
a = task_a.flow()           # TaskNode[str]
b = task_b.flow(input=a)    # 'a' is a TaskNode, dependency recorded
```

The `.flow()` method receives a TaskNode as `input`. It scans kwargs to find TaskNode dependencies.

During execution, we resolve TaskNodes to their actual results:

```python
def resolve_kwargs(kwargs: dict, results: dict[str, Result]) -> dict:
    """Replace TaskNodes with their actual results."""
    resolved = {}
    for k, v in kwargs.items():
        if isinstance(v, TaskNode):
            resolved[k] = results[v.node_id].unwrap()  # Get actual value
        else:
            resolved[k] = v
    return resolved
```

## Flow Execution Process

```python
@flow
def my_flow(*, source: Input[Path]):
    compiled = compile.flow(source=source)
    tested = test.flow(binary=compiled)
    return tested

# When my_flow(source=Path("src/")) is called:

1. Create new FlowPlan, set as current context
2. Run the flow function body:
   - compile.flow() creates TaskNode, adds to plan, returns it
   - test.flow() creates TaskNode with dependency on compiled, adds to plan
   - Flow returns the terminal TaskNode
3. FlowPlan now contains the full graph
4. Execute the plan in topological order:
   for node in plan.get_execution_order():
       resolved_kwargs = resolve_kwargs(node.kwargs, results)
       result = node.task_info.fn(**resolved_kwargs)  # Actually run task
       results[node.node_id] = result
       if result.failed:
           break  # Fail-fast
5. Return terminal node's result (or failure)
```

## API Surface

```python
@recompose.task
def compile(*, source: Path) -> recompose.Result[Path]:
    ...

@recompose.task
def test(*, binary: Path) -> recompose.Result[bool]:
    ...

@recompose.flow
def build_pipeline(*, source: Input[Path]) -> TaskNode[bool]:
    compiled = compile.flow(source=source)
    tested = test.flow(binary=compiled)
    return tested

# Execute (plan + run)
result = build_pipeline(source=Path("src/"))  # Returns Result[bool]

# Plan only (don't execute)
plan = build_pipeline.plan(source=Path("src/"))
print(plan.nodes)
print(plan.get_execution_order())
plan.visualize()  # Optional: show ASCII graph

# Dry run (plan + show what would happen)
build_pipeline.dry_run(source=Path("src/"))
```

## Type Safety

With the `.flow()` API, types are explicit and correct:

```python
@task
def compile(*, source: Path) -> Result[Path]: ...

# Direct call - clear types
result: Result[Path] = compile(source=Path("src/"))

# Flow variant - clear types
node: TaskNode[Path] = compile.flow(source=Path("src/"))

# Flow function signature uses Input[T] for parameters
@flow
def my_flow(*, source: Input[Path]) -> TaskNode[Path]:
    # source is Input[Path] = Path | TaskNode[Path]
    # Can accept literal Path from CLI or TaskNode from another task
    return compile.flow(source=source)
```

## Implementation Steps

1. **Create `Input[T]` type alias** - Union of `T | TaskNode[T]`
2. **Create `TaskNode[T]` class** - Generic, holds task info + kwargs + dependencies
3. **Create `FlowPlan` class** - Holds nodes, provides topological sort
4. **Add `.flow()` method to task wrapper** - Returns TaskNode, validates context
5. **Add `_current_plan` context variable** - Track active FlowPlan
6. **Update `@flow` decorator** - Create plan, run body, execute plan
7. **Add `plan()` method to flows** - Return FlowPlan without executing
8. **Add `dry_run()` method** - Show execution plan
9. **Error on wrong context** - `.flow()` outside flow, or direct task inside flow (optional)
10. **Tests** - Graph building, execution order, dependency resolution
11. **Update examples** - Show new capabilities

## Migration from P05a

The current imperative flows (P05a) will be deprecated. Migration:

```python
# Old (P05a) - imperative, auto-fail
@flow
def old_flow():
    check_prerequisites()  # Direct call, auto-fails
    run_linter()
    return Ok("done")

# New (P05b) - declarative with .flow()
@flow
def new_flow():
    prereq = check_prerequisites.flow()
    linted = run_linter.flow()
    return linted  # Return terminal node
```

## Completion Criteria

- [ ] `Input[T]` type alias works
- [ ] `TaskNode[T]` class implemented
- [ ] `FlowPlan` with topological sort
- [ ] `.flow()` method on tasks
- [ ] `@flow` decorator does plan-then-execute
- [ ] `flow.plan()` returns graph without executing
- [ ] Dependencies tracked through TaskNode kwargs
- [ ] Execution follows topological order
- [ ] Results passed between dependent tasks
- [ ] Tests for planning/graph features
- [ ] Example demonstrates the new API
