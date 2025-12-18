# P13: TaskClass in Flows

## Goal

Enable TaskClasses to be used as first-class entities in flows, where:
1. Instantiation (`Venv(location=...)`) creates a TaskNode in flows
2. Method calls (`venv.install_wheel(...)`) create TaskNodes in flows
3. TaskClass state is implicitly serialized after each method runs
4. TaskClasses can be passed to other tasks, which can use their methods as regular functions

## Motivating Example

**Current approach** (separate tasks, string paths):
```python
@recompose.flow
def wheel_test(*, full_tests: bool = False) -> None:
    wheel = build_wheel()
    venv = create_test_venv()
    install_wheel(wheel=wheel.value(), venv=venv.value())
    smoke_test(venv=venv.value())
    with recompose.run_if(full_tests):
        test_installed(venv=venv.value())
```

**New approach** (TaskClass with methods):
```python
@recompose.flow
def wheel_test(*, venv_location: Path = recompose.tempdir(), full_tests: bool = False) -> None:
    wheel = build_wheel()
    venv = Venv(location=venv_location)
    venv.install_wheel(wheel=wheel.value())
    smoke_test(venv=venv)  # Pass TaskClass directly (no .value() needed)
    with recompose.run_if(full_tests):
        test_installed(venv=venv)
```

The `smoke_test` task receives a `Venv` instance and can call `venv.run(...)` as a regular method.

## Design

### TaskClass Semantics

1. **`@taskclass` decorator on class** - Marks the class as a TaskClass
2. **`@task` decorator on methods** - Marks methods that become flow steps
3. **`__init__` is implicitly a task** - No decorator needed; instantiation becomes a step
4. **Non-decorated methods** - Regular methods, callable when TaskClass is passed to other tasks

### Dual Nature in Flows

When used in a flow context:
- `Venv(location=...)` returns a `TaskClassNode` (new type, similar to `TaskNode`)
- `venv.install_wheel(...)` returns a `TaskNode` for the method result
- The `TaskClassNode` tracks the latest method call for dependency ordering
- `TaskClassNode` is passed directly to tasks (no `.value()` needed)

### State Serialization

After any TaskClass method runs:
1. The method's return value is serialized (as today)
2. The TaskClass instance state is also serialized to workspace
3. Uses Pydantic for serialization (TaskClass should be a Pydantic model or dataclass)

When a TaskClass is passed to another task:
1. The receiving task deserializes the TaskClass from workspace
2. After the task completes, the TaskClass state is re-serialized (implicit tracking)

### Dependency Tracking

```python
venv = Venv(location=loc)           # step_01_venv.__init__
venv.install_wheel(wheel=w)         # step_02_venv.install_wheel, depends on step_01
smoke_test(venv=venv)               # step_03_smoke_test, depends on step_02 (not just step_01!)
```

The `TaskClassNode` must track its "current version" (latest step that modified it).

## Implementation Plan

### Phase 1: TaskClassNode Type

1. Create `TaskClassNode` in `plan.py`:
   - Generic over the TaskClass type: `TaskClassNode[Venv]`
   - Tracks the underlying class type
   - Tracks `init_node: TaskNode` for the instantiation step
   - Tracks `current_node: TaskNode` for the latest method call
   - Does NOT need Result-like interface (it's a node, not a result)

2. Update `taskclass` decorator:
   - Detect flow context during instantiation
   - Return `TaskClassNode` in flow context, actual instance otherwise

### Phase 2: Method Calls as TaskNodes

1. When `@task` method is called on a `TaskClassNode`:
   - Create a new `TaskNode` for the method
   - Add dependency on `current_node` of the `TaskClassNode`
   - Update `TaskClassNode.current_node` to the new node
   - Return `TaskNode` for the method result

2. Update TaskNode to track TaskClass context:
   - `taskclass_node: TaskClassNode | None` - The TaskClass this method belongs to
   - Used for serialization and dependency tracking

### Phase 3: State Serialization

1. Add serialization support for TaskClasses:
   - Require TaskClass to be a Pydantic BaseModel or dataclass
   - Serialize to `{workspace}/taskclass_{step_name}.json`

2. Update `local_executor.py`:
   - After method task completes, serialize TaskClass state
   - Before method task runs, deserialize TaskClass state from previous step

3. Update GHA generation:
   - Include TaskClass state serialization/deserialization in steps

### Phase 4: Passing TaskClass to Other Tasks

1. When a task receives a TaskClass parameter:
   - Detect `TaskClassNode` in kwargs during plan building
   - Add dependency on `current_node`
   - Mark the parameter for implicit state tracking

2. Implicit state serialization:
   - After task completes, serialize any TaskClass params
   - This handles cases where the task mutates the TaskClass

### Phase 5: Update Examples

1. Create `Venv` TaskClass:
   ```python
   @recompose.taskclass
   class Venv:
       location: Path

       def __init__(self, *, location: Path):
           self.location = location
           # Create venv...

       @recompose.task
       def install_wheel(self, *, wheel: str) -> recompose.Result[None]:
           # Install wheel...

       def run(self, *args: str) -> recompose.RunResult:
           # Run command in venv (regular method)
           python = self.location / "bin" / "python"
           return recompose.run(str(python), *args)
   ```

2. Create `Counter` TaskClass for testing:
   ```python
   @recompose.taskclass
   class Counter:
       count: int = 0

       def __init__(self, *, start: int = 0):
           self.count = start

       @recompose.task
       def increment(self, *, amount: int = 1) -> recompose.Result[int]:
           self.count += amount
           return recompose.Ok(self.count)
   ```

3. Update `wheel_test` flow to use `Venv` TaskClass

## Testing Strategy

1. **Unit tests for TaskClassNode**:
   - Creation in flow context
   - Method call creates TaskNode with correct dependencies
   - Result-like interface works

2. **Unit tests for state serialization**:
   - Counter TaskClass: increment, serialize, deserialize, verify count
   - Round-trip through workspace

3. **Integration tests**:
   - Flow with Counter: multiple increments, verify final state
   - Flow with Venv: create, install, smoke test
   - GHA generation produces correct YAML

4. **End-to-end**:
   - Run `wheel_test` flow locally
   - Verify it works the same as current implementation

## Completion Criteria

- [x] `TaskClassNode` type implemented
- [x] `@taskclass` returns `TaskClassNode` in flow context
- [x] Method calls on `TaskClassNode` create `TaskNode` with correct dependencies
- [x] TaskClass state serializes/deserializes correctly
- [x] Passing TaskClass to tasks works with implicit state tracking
- [x] `Counter` example works (stateful test case) - in test_taskclass_flow.py
- [ ] `Venv` TaskClass implemented
- [ ] `wheel_test` flow updated to use `Venv`
- [ ] GHA generation works for TaskClass flows
- [ ] All tests pass (193/193 passing)

## Notes

- TaskClasses should be simple data holders (Pydantic models or dataclasses)
- Complex state (like file handles) won't serialize - design for Path-based state
- The `Venv` case is easy because state is on filesystem, not in object
- The `Counter` case tests true in-object state serialization
