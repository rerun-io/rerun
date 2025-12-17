# Recompose Architecture

This document describes the architecture and organization of the recompose library.

## Overview

Recompose is a lightweight, typed, Pythonic task execution framework. It provides:
- **Tasks**: Single units of work (Python functions with `@task`)
- **Flows**: Compositions of tasks into dependency graphs (`@flow`)
- **Automations**: Higher-level orchestrations that dispatch flows (`@automation`)
- **GHA Integration**: Automatic generation of GitHub Actions workflows from flows

## Module Organization

```
src/recompose/
├── __init__.py           # Public API exports
│
├── # Core Abstractions
├── task.py               # @task decorator, TaskInfo, TaskWrapper
├── flow.py               # @flow decorator, FlowInfo, FlowWrapper
├── automation.py         # @automation decorator, AutomationInfo
├── result.py             # Result[T], Ok(), Err() - task return types
│
├── # Flow Graph Machinery
├── flowgraph.py          # FlowPlan, TaskNode, Input, InputPlaceholder
├── expr.py               # Expression AST for conditional logic (Expr, BinaryExpr, etc.)
├── conditional.py        # run_if() context manager for conditional execution
│
├── # Execution Context
├── context.py            # Ambient context: registries, debug mode, output helpers
├── workspace.py          # Subprocess isolation: params/results serialization
├── subprocess.py         # run() helper for spawning child processes
│
├── # Output & Rendering
├── output.py             # Tree-based output rendering (FlowRenderer)
│
├── # CLI
├── cli.py                # Click-based CLI generation from tasks/flows
├── command_group.py      # CommandGroup and Config dataclasses
├── builtin_tasks.py      # generate_gha, inspect built-in tasks
│
├── # GitHub Actions
├── gha.py                # Workflow YAML generation (WorkflowSpec, StepSpec, etc.)
├── github.py             # gh CLI wrapper (trigger workflows, check status)
```

## Core Concepts

### 1. Tasks (`task.py`)

A **task** is a Python function decorated with `@task`. Tasks:
- Return `Result[T]` to indicate success/failure with typed values
- Automatically catch exceptions and convert to `Err` results
- Detect if called inside a flow and return `TaskNode` for graph building
- Can be methods on classes via `@taskclass`

Key types:
- `TaskInfo`: Metadata about a task (name, signature, docstring, etc.)
- `TaskWrapper[P, T]`: Protocol for decorated task functions

### 2. Flows (`flow.py`, `flowgraph.py`)

A **flow** is a composition of tasks decorated with `@flow`. Flows:
- Build a task dependency graph at definition time
- Execute tasks in linear order (valid by construction)
- Support subprocess isolation (each task runs as separate process)
- Generate GitHub Actions workflows

Key types:
- `FlowInfo`: Metadata about a flow
- `FlowWrapper`: Protocol for decorated flow functions
- `FlowPlan`: The task dependency graph
- `TaskNode[T]`: A node in the graph representing a deferred task call
- `InputPlaceholder[T]`: Placeholder for flow parameters during GHA generation

### 3. Automations (`automation.py`)

An **automation** orchestrates multiple flows via `workflow_dispatch`. Automations:
- Use `flow.dispatch()` to trigger flows
- Generate "meta-workflows" that use `gh workflow run`

Key types:
- `AutomationInfo`: Metadata about an automation
- `AutomationPlan`: Tracks dispatched flows
- `FlowDispatch`: Represents a flow dispatch call

### 4. Results (`result.py`)

All tasks return `Result[T]`:
- `Ok(value)`: Success with a typed value
- `Err(message)`: Failure with error message
- Pydantic-based for serialization in workspace files

**Value access patterns:**
- `result.value()` - User-facing API for use inside tasks/flows. Raises `RuntimeError`
  if the result is a failure. This is the "fail fast" behavior users expect.
- `result._value` - Internal/framework access for inspection without triggering failure
  semantics (e.g., displaying output, serialization, conditional "is there a value?" checks).
  Framework code uses this when it needs to handle both success and failure cases gracefully.

### 5. Context (`context.py`)

The ambient context provides:
- **Task registries**: Populated by `main()` from explicit command list
- **Output helpers**: `out()`, `dbg()` for task output
- **Debug mode**: Enable verbose logging
- **Python command**: For GHA workflow generation (e.g., "uv run python")
- **Working directory**: For GHA workflows

### 6. Conditional Execution (`conditional.py`, `expr.py`)

The `run_if()` context manager enables conditional task execution:
```python
@flow
def my_flow(*, debug: bool = False):
    build()
    with run_if(debug):
        debug_task()  # Only runs if debug=True
```

This works both locally (condition evaluated at runtime) and in GHA (condition-check step outputs boolean, subsequent steps use `if:`).

Key types:
- `Expr`: Base class for condition expressions
- `InputExpr`, `LiteralExpr`, `BinaryExpr`, `UnaryExpr`: Expression types
- `ConditionalBlock`: Active conditional context

### 7. Workspace (`workspace.py`)

For subprocess isolation, flows use a workspace directory:
- `_params.json`: Flow parameters and step list
- `{step_name}.json`: Result from each step

The `Serializer` system handles custom type serialization.

### 8. CLI (`cli.py`, `command_group.py`)

Click-based CLI generation:
- `main()`: Entry point that builds CLI from explicit command list
- `CommandGroup`: Groups commands for organized `--help` output
- `Config`: GHA-related configuration (python_cmd, working_directory)

### 9. GHA Generation (`gha.py`)

Generates GitHub Actions workflow YAML:
- `WorkflowSpec`, `JobSpec`, `StepSpec`: Workflow structure
- `GHAAction`: Virtual tasks for `uses:` steps (checkout, setup-python, etc.)
- `render_flow_workflow()`: Convert flow to workflow spec
- `render_automation_workflow()`: Convert automation to workflow spec

### 10. GitHub CLI (`github.py`)

Wrapper around `gh` CLI for:
- Triggering `workflow_dispatch` events
- Checking workflow run status
- Validating local/remote workflow sync

## Data Flow

### Local Execution (Direct Call)
```
User calls task() → task wrapper executes function → returns Result[T]
```

### Local Execution (Flow)
```
User calls flow() →
  1. Build FlowPlan (task calls return TaskNode, added in execution order)
  2. Execute each task in order, resolve dependencies from previous results
  → returns Result[None]
```

Note: Nodes are added to the plan in valid execution order by construction - a task
can only use another task's result after that task has been called, so no explicit
topological sort is needed.

### Subprocess Isolation (run_isolated)
```
flow.run_isolated() →
  1. Build FlowPlan
  2. Create workspace, write _params.json
  3. For each step:
     - Spawn subprocess: `python app.py flow_name --step step_name`
     - Step reads params, executes task, writes {step_name}.json
  → returns Result[None]
```

### GHA Generation
```
generate_gha →
  1. Build FlowPlan with InputPlaceholders
  2. Inject setup_workspace step
  3. Inject condition-check steps
  4. Render to WorkflowSpec
  5. Write YAML to .github/workflows/
```

## Design Principles

1. **Tasks are just functions**: Minimal decoration, callable as normal Python
2. **CLI is opt-in**: `main()` builds CLI, but tasks work without it
3. **Result is explicit**: Tasks return `Result[T]` with value + status
4. **Context is ambient**: Helpers detect if running inside recompose engine
5. **Explicit registration**: Only commands passed to `main()` are CLI-accessible
6. **Local/CI parity**: Flows execute identically locally and in GHA

## Error Handling Conventions

**User-facing code (tasks, flows):**
- Tasks return `Result[T]` - use `Ok(value)` for success, `Err(message)` for failure
- The `@task` decorator catches uncaught exceptions and converts to `Err`
- Use `result.value()` to get the value - raises if failed (fail-fast for users)

**Internal framework code:**
- **Exceptions** for programming errors (invariants violated, setup not done)
- **`Result`** for expected/recoverable conditions (step not run yet, file might not exist)
- Use `result._value` for inspection without triggering failure semantics

Examples:
- `read_params()` raises `FileNotFoundError` - missing params is a programming error
- `read_step_result()` returns `Err` - step not run yet is an expected condition

## Common Patterns

### Adding a New Task
```python
@recompose.task
def my_task(*, param: str) -> recompose.Result[str]:
    recompose.out(f"Running with {param}")
    return recompose.Ok(f"done: {param}")

# Register in main()
commands = [recompose.CommandGroup("My Tasks", [my_task])]
recompose.main(commands=commands)
```

### Adding a New Flow
```python
@recompose.flow
def my_flow(*, config: str = "default") -> None:
    recompose.gha.checkout()
    result = my_task(param=config)
    another_task(input=result.value())

# Register in main()
commands = [recompose.CommandGroup("Flows", [my_flow])]
```

### Conditional Tasks
```python
@recompose.flow
def conditional_flow(*, run_optional: bool = False) -> None:
    required_task()
    with recompose.run_if(run_optional):
        optional_task()
```
