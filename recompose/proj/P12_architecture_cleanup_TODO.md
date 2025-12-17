# P12: Architecture Cleanup

## Overview

This project plan addresses code organization issues, naming inconsistencies, and code smells
identified during the architecture review. The goal is to make the codebase more approachable
and maintainable.

## Completion Criteria

- [ ] All modules have clear, single responsibilities
- [ ] Naming is consistent and intuitive (no gha/github confusion)
- [ ] Dead code and unused patterns removed
- [ ] Code duplication eliminated
- [ ] Imports are clean (no circular imports, minimal cross-dependencies)
- [ ] All tests pass

---

## 1. Module Naming Clarification: `gha.py` vs `github.py`

**Problem**: The distinction between `gha.py` and `github.py` is unclear.

**Current State**:
- `gha.py` (~840 lines): Workflow YAML generation, GHA actions (checkout, setup-python, etc.)
- `github.py` (~390 lines): `gh` CLI wrapper (trigger workflows, check status)

**Recommendation**: The naming is actually reasonable, but could be improved:
- Rename `gha.py` → `workflow.py` (it generates workflow specs)
- Keep `github.py` (it wraps the GitHub CLI)
- Alternative: Rename `github.py` → `gh_cli.py` to be more specific

**Tasks**:
- [ ] Decide on final naming
- [ ] Rename module(s)
- [ ] Update all imports
- [ ] Update documentation

**Effort**: Small

---

## 2. Module Naming Clarification: `flow.py` vs `flowgraph.py`

**Problem**: The distinction between `flow.py` and `flowgraph.py` is unclear.

**Current State**:
- `flow.py` (~530 lines): `@flow` decorator, `FlowInfo`, `FlowWrapper`, flow execution logic
- `flowgraph.py` (~600 lines): `FlowPlan`, `TaskNode`, `InputPlaceholder`, graph operations

**Analysis**: This split actually makes sense:
- `flow.py` is about the decorator and execution
- `flowgraph.py` is about the data structures for the graph

**Recommendation**: The naming could be clearer:
- Keep `flow.py` (decorator and execution)
- Rename `flowgraph.py` → `plan.py` or `graph.py` (emphasizes it's the plan/graph data structures)

**Tasks**:
- [ ] Decide on final naming
- [ ] Rename module
- [ ] Update all imports

**Effort**: Small

---

## 3. Consolidate Duplicate Code in `task.py` and `flow.py`

**Problem**: The wrapper creation logic in `task.py` (lines 131-203) and the taskclass wrapper
(lines 270-340) have significant duplication. Similarly, flow building has repeated patterns.

**Code Smell**: Both `task()` and `taskclass` create wrappers that:
1. Check if in flow-building mode (`get_current_plan()`)
2. Validate kwargs against signature
3. Create TaskNode if in flow mode
4. Execute task with context management if not

**Recommendation**: Extract common wrapper creation logic:
```python
def _create_task_wrapper(info: TaskInfo, execute_fn: Callable) -> Callable:
    """Create a wrapper that handles flow-mode detection and execution."""
    ...
```

**Tasks**:
- [ ] Extract common wrapper creation logic
- [ ] Refactor `task()` to use shared code
- [ ] Refactor `taskclass` to use shared code
- [ ] Ensure tests still pass

**Effort**: Medium

---

## 4. `flow.py` is Doing Too Much

**Problem**: `flow.py` has 530 lines handling:
- Flow decorator
- Flow context management
- Flow execution (`_execute_plan`)
- Subprocess isolation (`run_isolated_impl`)
- Tree output rendering integration
- Condition expression formatting

**Recommendation**: Split into focused modules:
- `flow.py`: Just the `@flow` decorator, `FlowInfo`, `FlowWrapper`
- `execution.py`: `_execute_plan`, `run_isolated_impl`, step execution logic
- Or: Keep `flow.py` but move `run_isolated_impl` to `workspace.py` since it's about subprocess isolation

**Tasks**:
- [ ] Identify clean boundaries
- [ ] Move execution logic to appropriate module
- [ ] Update imports
- [ ] Ensure tests pass

**Effort**: Medium

---

## 5. `cli.py` is Too Large (900+ lines)

**Problem**: `cli.py` has 900+ lines handling:
- Click command building (`_build_command`, `_build_flow_command`)
- Type conversion (`_get_click_type`)
- Flow execution modes (setup, step, remote, status)
- GitHub integration (`_handle_flow_status`, `_handle_flow_remote`)
- Registry building
- Grouped CLI generation

**Recommendation**: Split into focused modules:
- `cli.py`: Core CLI building (`_build_grouped_cli`, `main`)
- `cli_commands.py`: Individual command builders (`_build_command`, `_build_flow_command`)
- Move GitHub handling to `github.py` (or new `dispatch.py`)

**Tasks**:
- [ ] Identify clean boundaries
- [ ] Extract command builders
- [ ] Move GitHub handlers
- [ ] Update imports

**Effort**: Medium-Large

---

## 6. Remove Unnecessary Topological Sort

**Problem**: `FlowPlan.get_execution_order()` implements Kahn's algorithm for topological
sorting (~40 lines), but it's unnecessary.

**Analysis**: Nodes are added to `plan.nodes` in the order they're called during flow
function execution. Since Python executes sequentially and a TaskNode can only be used
*after* it's created, `self.nodes` is already in valid execution order by construction.

The topological sort produces the same result (or a different but still valid order for
independent tasks), but adds complexity without benefit.

**Current usage of `get_execution_order()`:**
- `flow.py:161` - `_execute_plan()` - could use `self.nodes` directly
- `cli.py:400` - `--setup` display - could use `self.nodes`
- `builtin_tasks.py:357` - `inspect` task - could use `self.nodes`

**Tasks**:
- [x] Replace `get_execution_order()` calls with `plan.nodes`
- [x] Remove `get_execution_order()` method
- [x] Remove `get_parallelizable_groups()` - removed along with `visualize()`
- [x] Update ARCHITECTURE.md (already didn't mention topological sort, just explains natural ordering)
- [x] Verify tests pass

**Effort**: Small

---

## 7. Unused/Vestigial Code in `workspace.py`

**Problem**: Backwards compatibility aliases that may no longer be needed:
```python
# Keep old names for backwards compatibility
_serialize_value = serialize_value
_deserialize_value = deserialize_value
```

**Tasks**:
- [x] Check if these aliases are used anywhere
- [x] Remove if unused

**Effort**: Trivial

---

## 8. Duplicate Git Root Finding

**Problem**: `_find_git_root()` is implemented in multiple places:
- `builtin_tasks.py:23-32`
- `github.py:296-308` (`find_git_root`)

**Recommendation**: Consolidate into `github.py` and import where needed.

**Tasks**:
- [x] Remove duplicate from `builtin_tasks.py`
- [x] Import from `github.py`
- [x] Update any direct subprocess calls to use shared function

**Effort**: Trivial

---

## 9. Context Module Has Too Many Globals

**Problem**: `context.py` has multiple module-level globals:
```python
_debug_mode: bool = False
_entry_point: tuple[str, str] | None = None
_python_cmd: str = "python"
_working_directory: str | None = None
```

These are all set by `main()` and accessed globally. While this works, it's fragile.

**Recommendation**: Consider consolidating into a single `RecomposeConfig` object:
```python
@dataclass
class RecomposeConfig:
    debug_mode: bool = False
    entry_point: tuple[str, str] | None = None
    python_cmd: str = "python"
    working_directory: str | None = None

_config: RecomposeConfig | None = None
```

**Tasks**:
- [ ] Decide if consolidation is worth the churn
- [ ] If yes, create config object and migrate
- [ ] Update all accessors

**Effort**: Medium (lots of call sites)

---

## 10. `gha.py` Virtual Task Factories

**Problem**: `setup_python()`, `setup_uv()`, etc. return `GHAAction` objects but are called
like they're tasks. The return type is inconsistent with their usage.

**Current**:
```python
def setup_python(version: str = "3.11", **kwargs: Any) -> GHAAction:
    return GHAAction("setup_python", ...)
```

**Usage in flows**:
```python
setup_python(version="3.11")()  # Returns Result[None] or TaskNode
# Or sometimes:
recompose.gha.setup_python("3.11")  # Relies on GHAAction.__call__
```

**Recommendation**: The pattern is actually fine - they're factory functions that return
callable objects. Just needs documentation.

**Tasks**:
- [x] Add docstring explaining the factory pattern
- [ ] Consider adding `@overload` for better type hints (deferred)

**Effort**: Trivial

---

## 11. Document Error Handling Convention

**Observation**: Some internal functions return `Result[T]` while others raise exceptions:
- `workspace.py:read_params()` raises `FileNotFoundError`
- `workspace.py:read_step_result()` returns `Err()`

**Analysis**: This is actually **intentional and correct**:
- `read_params()` missing = **programming error** (workspace not set up) → exception
- `read_step_result()` missing = **expected condition** (step not run yet) → `Err`

The pattern follows: exceptions for programming errors, `Result` for recoverable/expected errors.
This is internal framework code, not user-facing task code.

**Tasks**:
- [x] Document this convention in ARCHITECTURE.md (already done)
- [ ] Audit other internal functions to ensure they follow the same pattern (deferred)

**Effort**: Trivial (just documentation)

---

## 12. Test Coverage Gaps

**Current test files**:
- `test_task.py`, `test_flow.py`, `test_automation.py` - Core functionality
- `test_cli.py` - CLI generation
- `test_gha.py`, `test_github.py` - GHA integration
- `test_workspace.py` - Serialization
- `test_result.py`, `test_context.py`, `test_subprocess.py` - Utilities

**Missing/Light Coverage**:
- `conditional.py` / `expr.py` - No dedicated tests (tested through `test_gha.py`?)
- `output.py` - Tree rendering (visual, hard to test)
- `builtin_tasks.py` - `inspect` task

**Tasks**:
- [ ] Add tests for `conditional.py` / `expr.py`
- [ ] Add tests for `inspect` task
- [ ] Consider integration tests for full flow execution

**Effort**: Medium

---

## Priority Order

### Phase 1: Quick Wins (Low effort, high clarity) ✅ DONE
1. **#6**: Remove unnecessary topological sort - ✅ Done
2. **#7**: Unused backwards compatibility aliases - ✅ Done
3. **#8**: Duplicate git root finding - ✅ Done
4. **#10**: GHAAction documentation - ✅ Done
5. **#11**: Document error handling convention - ✅ Already in ARCHITECTURE.md

### Phase 2: Naming Clarity (Medium effort, high impact) ✅ DONE
6. **#1**: gha.py vs github.py naming - ✅ Done: kept gha.py, renamed github.py → gh_cli.py
7. **#2**: flow.py vs flowgraph.py naming - ✅ Done: renamed flowgraph.py → plan.py

### Phase 3: Code Organization (Medium-Large effort)
8. **#3**: Consolidate duplicate wrapper code - Reduces duplication
9. **#4**: Split flow.py - Clearer responsibilities
10. **#5**: Split cli.py - Clearer responsibilities

### Phase 4: Polish
11. **#9**: Context globals consolidation - Nice to have
12. **#12**: Test coverage - Ongoing

---

## Notes

- Each phase should be a separate PR for easier review
- Run full test suite after each change: `uv run pytest`
- Run linter after each change: `uv run ruff check`
- Update `ARCHITECTURE.md` if module structure changes
