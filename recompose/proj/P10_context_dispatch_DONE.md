# P10: Context-Based Flow Dispatch

## Goal

Remove the separate `.flow()` method and make tasks automatically detect whether they're being called in a flow-building context. This simplifies the API significantly - same function signature in both cases.

## Motivation

Currently:
```python
@recompose.flow
def my_flow():
    result = task_a.flow(arg="value")  # Must use .flow()
    task_b.flow(input=result.value())
```

Proposed:
```python
@recompose.flow
def my_flow():
    result = task_a(arg="value")  # Just call it!
    task_b(input=result.value())
```

**Benefits:**
1. **Simpler API** - No need to remember `.flow()` vs regular call
2. **Same signature** - Type system sees identical function signature in both modes
3. **Less code** - No need to inject/maintain `.flow()` method
4. **More intuitive** - Function "does the right thing" based on context
5. **Safer** - Impossible to accidentally call a task directly during flow compilation (since they're the same)

**Key insight:** Flow-compilation only happens in very specific, controlled circumstances (when `get_current_plan()` is not None). We can safely check this context in the task wrapper.

## Current Architecture

### Task wrapper (task.py:156-180)
```python
def wrapper(*args, **kwargs) -> Result[T]:
    # Check if we're inside a flow that's building a plan
    if get_current_plan() is not None:
        raise DirectTaskCallInFlowError(info.name)  # ← Prevent direct calls in flows

    # Normal execution logic...
    existing_ctx = get_context()
    if existing_ctx is None:
        ctx = Context(task_name=info.name)
        set_context(ctx)
        try:
            result = _execute_task(fn, args, kwargs)
        finally:
            set_context(None)
    else:
        result = _execute_task(fn, args, kwargs)
    return result
```

### Separate .flow() method (task.py:197-271)
```python
def flow_variant(**kwargs: Any) -> Any:
    plan = get_current_plan()
    if plan is None:
        raise RuntimeError(f"{info.name}.flow() can only be called inside a @flow")

    # Validate kwargs...
    # Create TaskNode...
    node = TaskNode(task_info=info, kwargs=kwargs, condition=condition)
    plan.add_node(node)
    return node

wrapper.flow = flow_variant  # ← Injected as method
```

## Proposed Architecture

### Unified wrapper
```python
def wrapper(*args, **kwargs) -> Result[T]:
    plan = get_current_plan()

    # IN FLOW-BUILDING MODE: Create TaskNode
    if plan is not None:
        # Validate kwargs (same as current .flow())
        # Create TaskNode and add to plan
        # Return TaskNode (mimics Result[T] for type checking)
        from .conditional import get_current_condition
        current_cond = get_current_condition()
        condition = current_cond.condition if current_cond else None

        node = TaskNode(task_info=info, kwargs=kwargs, condition=condition)
        plan.add_node(node)
        return node

    # NORMAL EXECUTION MODE: Execute task
    existing_ctx = get_context()
    if existing_ctx is None:
        ctx = Context(task_name=info.name)
        set_context(ctx)
        try:
            result = _execute_task(fn, args, kwargs)
        finally:
            set_context(None)
    else:
        result = _execute_task(fn, args, kwargs)
    return result
```

**No `.flow()` method needed!** The same wrapper does both jobs.

## Implementation Plan

### Phase 1: Core refactoring (code changes)
1. ✅ Create this plan document
2. ⬜ Modify `task()` decorator in `task.py`:
   - Move flow-building logic from `.flow()` into main wrapper
   - Remove `.flow()` method injection
   - Remove `DirectTaskCallInFlowError` (no longer needed)
3. ⬜ Modify `taskclass()` in `task.py`:
   - Apply same changes to method task wrappers
4. ⬜ Update `TaskWrapper` protocol to remove `.flow()` method signature
5. ⬜ Remove `DirectTaskCallInFlowError` from `flow.py`

### Phase 2: Update all callsites (256 occurrences)
**Examples:**
- `examples/tutorial/intro_flows.py` - 25 calls
- `examples/flows/ci.py` - 7 calls
- `examples/flows/wheel_test.py` - 5 calls

**Tests:**
- `tests/test_declarative_flow.py` - 34 calls
- `tests/test_parameterized_flows.py` - 23 calls
- `tests/test_flow.py` - 18 calls
- `tests/test_gha.py` - 10 calls
- `tests/test_workspace.py` - 9 calls
- `tests/test_automation.py` - 2 calls

**Source:**
- `src/recompose/gha.py` - 7 calls (GHA action helpers)
- `src/recompose/conditional.py` - 5 calls (run_if tests)
- `src/recompose/flow.py` - 10 calls (internal)
- `src/recompose/flowgraph.py` - 22 calls (likely docstrings/comments)
- `src/recompose/task.py` - 18 calls (likely docstrings/comments)

**Documentation:**
- `WORK.md` - 8 calls
- `examples/README.md` - 12 calls
- Various `proj/*.md` files

### Phase 3: Update documentation
1. ⬜ Update docstrings in `task.py` to explain context-based dispatch
2. ⬜ Update tutorial comments in `examples/tutorial/intro_flows.py`
3. ⬜ Update `examples/README.md`
4. ⬜ Update `WORK.md` and `PLAN.md` if needed

### Phase 4: Validation
1. ⬜ Run all tests: `./run test`
2. ⬜ Run all examples manually to verify behavior
3. ⬜ Check type checking still works (mypy)
4. ⬜ Verify GHA workflow generation still works

## Edge Cases to Consider

1. **Type checking**: The wrapper returns `TaskNode[T] | Result[T]` depending on context, but to the type checker it should always look like `Result[T]`. This already works (TaskNode mimics Result).

2. **Nested flows**: What if a flow calls another flow? This should work - the inner flow would be treated as a single node in the outer flow's plan.

3. **Method tasks**: Need to ensure `@taskclass` method wrappers get the same treatment.

4. **Error messages**: Update error messages to reflect that tasks can be called directly (no more "use .flow()" suggestions).

5. **Validation**: The current `.flow()` does kwargs validation. Need to preserve this in the flow-building branch.

## Testing Strategy

1. **Existing tests should mostly pass** after mechanical `.flow()` removal
2. **Add new tests** for context-based dispatch:
   - Verify task callable directly outside flow
   - Verify task callable inside flow (creates TaskNode)
   - Verify error if task called with wrong args in flow
3. **Type checking tests** to ensure `Result[T]` signature preserved

## Rollout

This is a **breaking API change** for any external users. However:
- Project is in early development (0.1.0)
- No external users yet
- Simplification is worth it now vs later

## Completion Criteria

- [ ] All 256 `.flow()` calls removed
- [ ] All tests passing
- [ ] Type checking passes
- [ ] GHA workflow generation works
- [ ] Examples run successfully
- [ ] Documentation updated

## Risks

**Low risk:**
- Changes are mechanical and well-defined
- Tests provide good coverage
- Context variable approach is clean and isolated

**Main risk:**
- Missing some `.flow()` calls in the refactoring → Mitigated by running tests and examples
- Type checking regression → Mitigated by running mypy
