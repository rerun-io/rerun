# P04: Member Tasks

**Status:** IN_PROGRESS
**Goal:** Support `@task` decorator on class methods.

## Overview

Allow classes to have task-decorated methods. When invoked via CLI:
- Class `__init__` args are combined with method args
- The object is constructed first, then the method is called
- Naming convention: `classname.methodname` (e.g., `venv.sync`)

## Design

### Usage Example

```python
import recompose
from pathlib import Path

class Venv:
    def __init__(self, *, location: Path, clean: bool = False):
        self.location = location
        if clean and location.exists():
            shutil.rmtree(location)
        # Create venv...

    @recompose.task
    def sync(self, *, group: str | None = None) -> recompose.Result[None]:
        """Sync dependencies."""
        recompose.out(f"Syncing venv at {self.location}")
        recompose.run("uv", "sync", cwd=self.location)
        return recompose.Ok(None)

    @recompose.task
    def run(self, *, cmd: str) -> recompose.Result[int]:
        """Run a command in the venv."""
        result = recompose.run(self.location / "bin" / "python", "-c", cmd)
        return recompose.Ok(result.returncode)
```

### CLI Exposure

```bash
# List available commands
./app.py --help
# Shows: venv.sync, venv.run

# Call venv.sync - constructs Venv then calls sync()
./app.py venv.sync --location=/tmp/myvenv --group=dev

# Call venv.run
./app.py venv.run --location=/tmp/myvenv --cmd="print('hello')"
```

### Registration Mechanism

When `@task` decorates a method:
1. Detect it's an unbound method (first param is `self`)
2. Store metadata about the class and method
3. At CLI build time, introspect the class `__init__` signature
4. Combine `__init__` args + method args into single command

### Key Design Decisions

1. **`__init__` is NOT a task** - It's just a regular constructor. Only methods get `@task`.
2. **Dot notation for names** - `classname.methodname` keeps it flat but clear.
3. **All `__init__` args must be keyword-only** - For clean CLI mapping.
4. **Instance is ephemeral** - Created fresh for each CLI invocation.

### Implementation Approach

**Option A: Descriptor-based**
- `@task` returns a descriptor that captures method + class info
- At class definition time, `__init_subclass__` or metaclass collects tasks
- Requires class cooperation (inherit from base or use decorator)

**Option B: Deferred registration**
- `@task` on methods stores metadata on the function
- A separate `@recompose.taskclass` decorator on the class triggers registration
- Scans class for task-decorated methods and registers them

**Option C: Manual registration**
- User explicitly calls `recompose.register_class(Venv)` after class definition
- Simplest implementation, most explicit

Going with **Option B** - it's explicit but not too verbose:

```python
@recompose.taskclass
class Venv:
    def __init__(self, *, location: Path):
        self.location = location

    @recompose.task
    def sync(self, *, group: str | None = None) -> recompose.Result[None]:
        ...
```

## Implementation Steps

1. **Add `@taskclass` decorator** - Scans class for `@task` methods
2. **Extend TaskInfo** - Add fields for class-based tasks (cls, is_method, init_signature)
3. **Update `@task` for methods** - Detect unbound methods, store differently
4. **Update CLI builder** - Handle class tasks: combine init + method args
5. **Tests** - Class-based task registration, CLI invocation, arg combining
6. **Example** - Demonstrate with a realistic use case

## Completion Criteria

- [x] `@taskclass` decorator works
- [x] `@task` on methods registers correctly
- [x] CLI shows `classname.methodname` commands
- [x] Combined args from `__init__` + method work
- [x] Object is constructed then method called
- [x] Tests pass (8 new tests)
- [x] Example demonstrates the feature (member_tasks_demo.py)
