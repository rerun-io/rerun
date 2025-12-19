# P16: Unified Task Output System

**Status: DONE**

## Summary

Created a unified `OutputManager` class that handles all output formatting for tasks, automations, and steps with consistent tree-style hierarchy, colors, and GHA compatibility.

## What Was Implemented

### Phase 1: OutputManager Class (output.py)
- Created `OutputManager` dataclass with rich Console integration
- Implemented `PrefixWriter` for tree-style output prefixing
- Added `ScopeInfo` for tracking nested scopes with timing
- Implemented context managers: `task_scope()`, `nested_task_scope()`, `job_scope()`, `step_scope()`, `parallel_scope()`
- Added output methods: `task_header()`, `task_status()`, `job_header()`, `job_status()`, `automation_header()`, `automation_status()`
- GHA detection via `GITHUB_ACTIONS` environment variable
- Global singleton access via `get_output_manager()`

### Phase 2: Nested Task Output (task.py)
- Removed old `_TreePrefixWriter` class
- Updated `_run_nested_task()` to use `OutputManager.nested_task_scope()`
- Proper error detail display on failure
- Status correctly reflects Result success/failure

### Phase 3: Automation Executor (local_executor.py)
- Refactored to use `OutputManager` for all output
- `automation_header()` and `automation_status()` for automation scope
- `job_header()` and `job_status()` for individual jobs
- `parallel_header()` for parallel job groups
- Proper symbols: `▼` for entry, `├─▶` for branch, `└─▶` for last, `⊕─┬─▶` for parallel
- **Parent-side prefixing**: Subprocess output is captured by parent and prefixed at parent level
  - This composes naturally for arbitrary nesting depth
  - No env vars needed to coordinate indentation between processes

### Phase 4: Step Integration
- Kept existing step.py working
- OutputManager has `step_scope()` context manager available for future use
- Not heavily refactored since step functionality is working as-is

## Symbology Implemented

| Symbol | Meaning |
|--------|---------|
| `▼` | Top-level entry point |
| `│` | Continuation line (main backbone) |
| `├─▶` | Sequential item (not last) |
| `└─▶` | Last item in group |
| `⊕─┬─▶` | Parallel fork start |
| `│ ├─▶` | Parallel branch item |
| `│ └─▶` | Last parallel item |
| `✓` | Success (green) |
| `✗` | Failure (red) |

## Output Examples

### Nested Tasks (`./run lint-all`)

```
▶ lint_all

├─▶ lint
│    Running ruff check...
│    All checks passed!
│    Running mypy...
│    Success: no issues found
│    ✓ 0.35s
├─▶ format_check
│    Checking code formatting...
│    ✓ 0.02s
├─▶ generate_gha
│    Checking 3 workflow(s)...
│    All workflows up-to-date!
│    ✓ 0.01s
All lint checks passed!

✓ lint_all succeeded in 0.38s
```

### Automation with Parallel Jobs (`./run ci --verbose`)

```
▼ ci
│
⊕─┬─▶ Running in parallel: lint_all, test
│ ├─▶ lint_all
│    ├─▶ lint
│    │    Running ruff check...
│    │    All checks passed!
│    │    Running mypy...
│    │    Success: no issues found
│    │    ✓ 0.18s
│    ├─▶ format_check
│    │    Checking code formatting...
│    │    ✓ 0.02s
│    ├─▶ generate_gha
│    │    Checking 3 workflow(s)...
│    │    ✓ 0.01s
│    All lint checks passed!
✓ 0.33s
│ └─▶ test
│    Running tests...
│    ============================= test session starts ==============================
│    ...
│    ======================== 234 passed, 1 skipped in 0.20s ========================
✓ 0.44s

✓ ci completed in 0.44s (2 jobs)
```

### Phase 5: Polish (subprocess.py, Rich cleanup)
- Removed redundant `│` prefix from `subprocess.py` - PrefixWriter handles all prefixing now
- Disabled Rich auto-highlighting in `_print_raw()` to prevent ANSI code pollution
- Clean, consistent output with proper styling

## Files Modified

- `src/recompose/output.py` - Completely rewritten with new OutputManager
- `src/recompose/task.py` - Updated to use OutputManager for nested tasks
- `src/recompose/local_executor.py` - Refactored to use OutputManager
- `src/recompose/subprocess.py` - Removed redundant tree-mode prefix handling

## Completion Criteria

- [x] Single `OutputManager` handles all output formatting
- [x] Nested task output has colors and consistent styling
- [x] Automation job output uses same visual style
- [x] GHA mode works with ::group:: markers
- [x] All existing tests pass (234 passed)
- [x] `./run lint-all` shows colored tree output
- [x] `./run ci` shows colored parallel job output

## Known Limitations / Future Work

1. **Parallel job output buffering**: When jobs run in parallel, their output is buffered and printed after completion (in order). This is necessary to prevent interleaving, but means you don't see live output for parallel jobs.

2. **Step integration**: The step.py module wasn't fully refactored to use OutputManager. It works as-is, but could be unified further if needed.

3. **out()/dbg() integration**: These context helpers still use simple print() rather than going through OutputManager. Could be unified if needed.

## Key Design Decision

**Parent-side prefixing**: Subprocess output is captured by the parent process and prefixed at the parent's level. This was chosen over having subprocesses manage their own indentation because:
- It composes naturally for arbitrary nesting depth
- No environment variable coordination needed between processes
- Simpler to understand and maintain
- Works with any subprocess (not just recompose tasks)
