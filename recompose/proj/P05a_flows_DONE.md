# P05: Flows (Task Composition)

**Status:** IN_PROGRESS
**Goal:** Compose tasks into executable sequences/graphs.

## Incremental Approach

Breaking this into sub-phases:

### P05a: Simple Sequential Flows (MVP)
Compose tasks that run sequentially in the same process. Results from one task can be passed to the next.

```python
@recompose.flow
def build_and_test() -> recompose.Result[str]:
    # Tasks execute in sequence, results are passed through
    build_result = build_project()
    if build_result.failed:
        return build_result

    test_result = run_tests()
    return test_result
```

**Key features:**
- `@flow` decorator marks a function as a flow
- Flow contains calls to `@task` functions
- Tasks run sequentially in the same process
- Flow is also exposed as a CLI command
- Early exit on failure (configurable)

### P05b: Subprocess Isolation
Each task runs in its own subprocess. Results are serialized/deserialized.

```python
@recompose.flow(subprocess=True)
def isolated_flow() -> recompose.Result[str]:
    # Each task runs in a subprocess
    result1 = task_one()  # subprocess 1
    result2 = task_two(input=result1.value)  # subprocess 2
    return result2
```

**Key features:**
- Tasks invoked via CLI in subprocess
- Results serialized to JSON (using pydantic)
- Environment/cwd passed to subprocesses
- Better isolation, matches CI behavior

### P05c: DAG and Parallelization (Future)
Proper dependency graph with parallel execution of independent tasks.

---

## P05a Implementation Plan

### Design

**Flow decorator:**
```python
@recompose.flow
def my_flow(*, some_input: str) -> recompose.Result[str]:
    """A flow that runs multiple tasks."""
    r1 = first_task(input=some_input)
    if r1.failed:
        return r1

    r2 = second_task(value=r1.value)
    return r2
```

**What the decorator does:**
1. Wraps the function similar to `@task`
2. Registers it in a flow registry (or the same task registry with a flag)
3. Exposes it as a CLI command
4. Provides flow-specific context (tracking which tasks ran, timing, etc.)

**FlowResult:**
Extends Result to include information about sub-tasks:
```python
@dataclass
class FlowResult(Result[T]):
    task_results: list[tuple[str, Result]]  # (task_name, result) pairs
    total_duration: float
```

### Implementation Steps

1. **Create `flow.py` module** with:
   - `@flow` decorator
   - `FlowContext` for tracking execution
   - `FlowResult` type

2. **Update registry** to handle flows (or use same registry with `is_flow` flag)

3. **Update CLI** to expose flows as commands

4. **Tests** for flow execution, result passing, error handling

5. **Example** demonstrating a realistic flow

### API Surface

```python
# In __init__.py
from .flow import flow, FlowResult

# Usage
@recompose.flow
def my_flow(*, arg: str) -> recompose.Result[str]:
    ...
```

## Completion Criteria (P05a)

- [x] `@flow` decorator works
- [x] Flows can call tasks sequentially
- [x] Results pass between tasks
- [x] Flows appear in CLI (with [flow] prefix)
- [x] FlowContext tracks sub-task results (attached to Result)
- [x] Tests pass (10 new tests)
- [x] Example demonstrates usage (flow_demo.py)

**P05a COMPLETE** - Basic sequential flows working. P05b (subprocess isolation) and P05c (DAG) are future work.
