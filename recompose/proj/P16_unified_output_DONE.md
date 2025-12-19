# P16: Unified Task Output System

**Status: DONE**

## Summary

Created a unified `OutputManager` class that handles all output formatting for tasks, automations, and steps with consistent tree-style hierarchy, colors, and GHA compatibility.

## What Was Implemented

### Core Design: Simple Recursive Output Model

Each execution level follows the same pattern:
1. Parent prints child's header (├─▶ or └─▶)
2. Parent executes child, capturing ALL output
3. Parent prefixes ALL captured output with continuation prefix
4. Parent prints status with SAME prefix
5. Move to next child

This composable approach eliminates ~550 lines of complexity and works naturally
for arbitrary nesting depth without special cases for parallel execution.

### output.py - Simple OutputManager
- `prefix_lines()` function to add prefix to each line of text
- `CONTENT_PREFIX = "│   "` (4 chars) for non-last siblings
- `LAST_PREFIX = "    "` (4 chars) for last sibling
- Simple methods: `print_header()`, `print_status()`, `get_continuation_prefix()`, `print_prefixed()`
- GHA detection via `GITHUB_ACTIONS` environment variable
- Global singleton via `get_output_manager()`

### task.py - Nested Task Output
- `_run_nested_task()` captures all output and prefixes it
- Uses same recursive model as other execution levels
- Status printed with same prefix as content

### local_executor.py - Automation Executor
- `JobResult` has `output_text` field for captured subprocess output
- `_execute_job()` captures all subprocess output
- `_print_job_result()` implements the recursive model
- Parallel jobs run with ThreadPoolExecutor, results printed sequentially

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

## Files Modified

- `src/recompose/output.py` - Simplified to ~100 lines with `prefix_lines()` and simple OutputManager
- `src/recompose/task.py` - Updated `_run_nested_task()` to capture and prefix output
- `src/recompose/local_executor.py` - Refactored with recursive output model

## Completion Criteria

- [x] Single `OutputManager` handles all output formatting
- [x] Nested task output has colors and consistent styling
- [x] Automation job output uses same visual style
- [x] GHA mode works with ::group:: markers
- [x] All existing tests pass (234 passed)
- [x] `./run lint-all` shows colored tree output
- [x] `./run ci` shows colored parallel job output

## Key Design Decision

**Simple Recursive Model with Parent-Side Prefixing**: Each execution level captures its
children's output and prefixes it uniformly. This was chosen over complex scope tracking
and subprocess-managed indentation because:
- Composes naturally for arbitrary nesting depth
- No special cases for parallel vs sequential execution
- No environment variable coordination between processes
- Simpler to understand and maintain (~550 lines removed)
- Works with any subprocess (not just recompose tasks)

## Known Limitations / Future Work

1. **Parallel job output buffering**: When jobs run in parallel, their output is buffered
   and printed after completion (in order). This prevents interleaving but means no live
   output for parallel jobs.

2. **out()/dbg() integration**: These context helpers still use simple print() rather than
   going through OutputManager. Could be unified if needed.
