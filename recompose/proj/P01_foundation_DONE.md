# P01: Foundation

**Status:** DONE
**Goal:** Working package with `@task` decorator and basic `Result` type.

## Scope

This is the MVP core. By the end, we should be able to:

```python
import recompose

@recompose.task
def greet(*, name: str, count: int = 1) -> recompose.Result[str]:
    for _ in range(count):
        recompose.out(f"Hello, {name}!")
    return recompose.Ok("done")

# Can call directly as a function:
result = greet(name="World", count=3)
assert result.ok
assert result.value == "done"
```

## Tasks

- [x] Create pyproject.toml with uv
- [x] Create src/recompose/__init__.py with public API
- [x] Create src/recompose/result.py - Result[T] type
- [x] Create src/recompose/context.py - Context, out(), dbg()
- [x] Create src/recompose/task.py - @task decorator and registry
- [x] Create basic tests
- [x] Verify package installs and imports correctly

## Implementation Notes

### Result Type (Immutable Factory Pattern)

```python
from typing import Generic, TypeVar, Literal
from pydantic import BaseModel

T = TypeVar("T")

class Result(BaseModel, Generic[T]):
    """Base result type. Use Ok(value) or Err(message) to construct."""
    value: T | None = None
    status: Literal["success", "failure"] = "success"
    error: str | None = None
    # Later: captured_output, duration, traceback, etc.

    @property
    def ok(self) -> bool:
        return self.status == "success"

    @property
    def failed(self) -> bool:
        return self.status == "failure"


def Ok(value: T) -> Result[T]:
    """Create a successful result."""
    return Result(value=value, status="success")


def Err(error: str, value: T | None = None) -> Result[T]:
    """Create a failed result."""
    return Result(value=value, status="failure", error=error)
```

### Context

The context is stored in a ContextVar so it's thread/async safe. When running outside recompose, helpers fall back to simple behavior.

```python
from contextvars import ContextVar

_current_context: ContextVar["Context | None"] = ContextVar("recompose_context", default=None)

def out(message: str) -> None:
    ctx = _current_context.get()
    if ctx:
        ctx.output.append(("out", message))
    print(message)  # Always print for now

def dbg(message: str) -> None:
    ctx = _current_context.get()
    if ctx:
        ctx.output.append(("dbg", message))
    # Only print in debug mode or when running in recompose
```

### Task Decorator

The decorator should:
1. Register the function in a global registry
2. Wrap it to optionally manage context
3. Preserve the original function signature

```python
_task_registry: dict[str, TaskInfo] = {}

def task(fn: Callable[P, Result[T]]) -> Callable[P, Result[T]]:
    """Decorator to mark a function as a recompose task."""
    info = TaskInfo(
        name=fn.__name__,
        module=fn.__module__,
        fn=fn,
        signature=inspect.signature(fn),
    )
    _task_registry[f"{fn.__module__}:{fn.__name__}"] = info

    @functools.wraps(fn)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> Result[T]:
        # Set up context if not already in one
        ctx = _current_context.get()
        if ctx is None:
            ctx = Context(task_name=info.name)
            token = _current_context.set(ctx)
            try:
                return fn(*args, **kwargs)
            finally:
                _current_context.reset(token)
        else:
            return fn(*args, **kwargs)

    return wrapper
```

## Design Decisions (Resolved)

1. **Result is immutable** - Use `Ok(value)` and `Err(message)` factory functions.

2. **Exceptions are caught** - The `@task` wrapper catches exceptions and converts to `Err` results with traceback.

## Definition of Done

- [x] `uv run python -c "import recompose"` works
- [x] Can define a task with @recompose.task
- [x] Can call task as normal function
- [x] Task returns Result with value and status
- [x] recompose.out() prints and captures output
- [x] Basic test suite passes (24 tests)
