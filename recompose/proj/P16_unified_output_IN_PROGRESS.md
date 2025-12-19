# P16: Unified Task Output System

## Problem

We currently have multiple overlapping output mechanisms:

1. **`local_executor.py`** - Automation job output with rich console colors
2. **`task.py`** - `_TreePrefixWriter` for nested tasks (no colors, just added)
3. **`step.py`** - `StepOutputWrapper` for step indentation within tasks
4. **`context.py`** - `out()` and `dbg()` functions

These don't play nicely together:
- Nested task output lost colors when I added tree prefixes
- Automation executor and nested task execution have different code paths
- Step indentation vs task indentation are separate systems
- No unified way to control verbosity, colors, etc.

## Requirements

### Core Behaviors

1. **Hierarchical output** - Both automations (subprocess) and nested tasks (in-process) should show tree-style hierarchy
2. **Consistent styling** - Colors, symbols (✓, ✗, ▶, ├─▶), timing should be uniform
3. **GHA compatibility** - Detect GHA environment and use `::group::`/`::endgroup::` markers instead of tree formatting
4. **Verbosity control** - Support quiet/normal/verbose modes
5. **Output capture** - Subprocess output should be captured and re-emitted with proper prefixes

### Execution Modes

| Mode | Description | Output Style |
|------|-------------|--------------|
| Top-level task | `./run lint` | Header, output, status |
| Nested task | Task calls task directly | Tree-indented with `│` prefix |
| Automation job | Subprocess via executor | Tree-indented, buffered for parallel |
| Step within task | `with step("name"):` | Indented grouping |

## Proposed Architecture

### Single Output Manager

Create a unified `OutputManager` class that handles all output formatting:

```python
class OutputManager:
    """Centralized output formatting for recompose."""

    def __init__(self):
        self.console = Console()  # rich console
        self._depth = 0  # nesting depth
        self._is_gha = os.environ.get("GITHUB_ACTIONS") == "true"
        self._verbosity = Verbosity.NORMAL

    # Context managers for nesting
    def task_scope(self, name: str) -> ContextManager
    def step_scope(self, name: str) -> ContextManager
    def job_scope(self, name: str, parallel: bool = False) -> ContextManager

    # Output methods
    def task_header(self, name: str)
    def task_status(self, success: bool, elapsed: float)
    def line(self, message: str, style: str = None)
    def error(self, message: str)

    # Prefix management
    def current_prefix(self) -> str  # e.g., "│    │    "
    def wrap_stdout(self) -> ContextManager  # wraps stdout with prefix
```

## Symbology

| Symbol | Meaning |
|--------|---------|
| `▼` | Top-level entry point (execution flows down from here) |
| `│` | Main execution backbone |
| `⊕─┬─▶` | Parallel fork (multiple jobs run concurrently) |
| `├─▶` | Sequential job (single job on backbone) |
| `└─▶` | Last item in a parallel group |
| `✓` | Success (green) |
| `✗` | Failure (red) |

### Mockup: Nested Tasks

```
▼ lint_all
│
├─▶ lint
│    Running ruff check...
│    All checks passed!
│    Running mypy...
│    Success: no issues found
│    ✓ 0.37s
│
├─▶ format_check
│    Checking code formatting...
│    ✓ 0.02s
│
├─▶ generate_gha
│    Checking 3 workflow(s)...
│    All workflows up-to-date!
│    ✓ 0.01s
│
✓ lint_all succeeded in 0.41s
```

### Mockup: Automation with Parallel Jobs (Single Wave)

```
▼ ci
│
⊕─┬─▶ lint_all
│ │    ├─▶ lint
│ │    │    Running ruff check...
│ │    │    ✓ 0.30s
│ │    ├─▶ format_check
│ │    │    ✓ 0.02s
│ │    ✓ 0.35s
│ │
│ └─▶ test
│      Running pytest...
│      ✓ 0.48s
│
✓ ci completed in 0.50s (2 jobs)
```

### Mockup: Multiple Waves (Dependencies)

```
▼ build_test_deploy
│
⊕─┬─▶ lint
│ │    ✓ 0.3s
│ └─▶ format_check
│      ✓ 0.2s
│
⊕─┬─▶ unit_test
│ │    ✓ 1.0s
│ └─▶ integration_test
│      ✓ 2.0s
│
├─▶ deploy
│    Deploying to production...
│    ✓ 1.2s
│
✓ build_test_deploy completed in 3.5s
```

### Mockup: Failure Case

```
▼ lint_all
│
├─▶ lint
│    Running ruff check...
│    All checks passed!
│    Running mypy...
│    src/foo.py:10: error: Incompatible types
│    Found 1 error
│    ✗ 0.62s
│
✗ lint_all failed in 0.62s
```

### Mockup: GHA Mode

In GitHub Actions, no tree formatting - use collapsible groups:

```
::group::lint
Running ruff check...
All checks passed!
Running mypy...
Success: no issues found
✓ lint completed in 0.37s
::endgroup::

::group::format_check
Checking code formatting...
✓ format_check completed in 0.02s
::endgroup::

✓ lint_all succeeded in 0.41s
```

## Implementation Plan

### Phase 1: Create OutputManager

1. Create `src/recompose/output.py` with `OutputManager` class
2. Single global instance accessed via `get_output_manager()`
3. Support both rich console (local) and plain text (GHA/subprocess)

### Phase 2: Unify Task Execution Output

1. Remove `_TreePrefixWriter` from `task.py`
2. Have `_run_nested_task` use `OutputManager.task_scope()`
3. Ensure colors work via rich console

### Phase 3: Unify Automation Executor

1. Refactor `local_executor.py` to use `OutputManager`
2. Job output goes through same formatting pipeline
3. Parallel job buffering still works but uses unified output

### Phase 4: Integrate Step System

1. Have `step()` context manager use `OutputManager.step_scope()`
2. Remove redundant `StepOutputWrapper` from `step.py`

### Phase 5: Testing and Polish

1. Test all combinations: top-level task, nested task, automation, steps
2. Test GHA mode detection
3. Ensure subprocess output capture still works

## Key Design Decisions

1. **Rich Console for styling** - Use rich library for colors/formatting, not ANSI codes
2. **Single source of truth** - One `OutputManager` handles all output decisions
3. **Context-based nesting** - Use context managers to track depth, not global state scattered everywhere
4. **GHA detection** - Check once at startup, use consistently throughout
5. **Subprocess handling** - Subprocesses write plain text, parent re-formats with prefixes

## Open Questions

1. Should `out()` and `dbg()` go through OutputManager or remain separate?
2. How to handle verbose mode for automation jobs (currently shows all output)?
3. Should we support NO_COLOR environment variable?

## Files to Modify

- `src/recompose/output.py` (new)
- `src/recompose/task.py` - use OutputManager for nested tasks
- `src/recompose/step.py` - use OutputManager for steps
- `src/recompose/local_executor.py` - use OutputManager for automation output
- `src/recompose/context.py` - possibly route out()/dbg() through OutputManager

## Completion Criteria

- [ ] Single `OutputManager` handles all output formatting
- [ ] Nested task output has colors and consistent styling
- [ ] Automation job output uses same visual style
- [ ] GHA mode works with ::group:: markers
- [ ] All existing tests pass
- [ ] `./run lint-all` shows colored tree output
- [ ] `./run ci` shows colored parallel job output
