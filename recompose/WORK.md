# NOW

**P10_context_dispatch** - COMPLETE

Simplified the API by removing the `.flow()` method - tasks now use context-based dispatch.

Tasks automatically detect whether they're being called inside a flow-building
context and behave accordingly. This eliminates API duplication and makes the
code more intuitive.

**Key changes:**
- Removed `.flow()` method from tasks
- Tasks check `get_current_plan()` and dispatch to either:
  - Flow-building mode: Return TaskNode for graph construction
  - Execution mode: Execute the task and return Result
- Updated TaskWrapper protocol (no longer has `.flow()` method)
- Removed `DirectTaskCallInFlowError` (no longer needed)
- Updated all 256 `.flow()` calls across examples, tests, and source code
- GHAAction uses same context-based dispatch pattern

**Benefits:**
- Simpler API - one less thing to remember
- Same function signature everywhere
- Cleaner code - no `.flow()` noise
- Type-safe - same Result[T] signature in both modes

All 181 tests passing, lint clean.

See: `proj/P10_context_dispatch_DONE.md` for detailed plan

---

**P09_workflow_dispatch** - COMPLETE

Implemented ergonomic CLI-to-GitHub integration for flows.

**New CLI options for flows:**
- `--remote` - Trigger workflow on GitHub instead of running locally
- `--status` - Show recent GitHub Actions runs for the flow
- `--force` - Skip workflow sync validation (with `--remote`)
- `--ref` - Specify branch/tag to run against (default: current branch)

**Example usage:**
```bash
# Show recent runs for the ci flow
./run ci --status

# Trigger ci flow on GitHub (validates workflow is in sync)
./run ci --remote

# Force trigger even if workflow differs
./run ci --remote --force

# Trigger on a specific branch
./run ci --remote --ref main
```

**Implementation:**
- `src/recompose/github.py` - GitHub CLI wrapper module
  - `list_workflow_runs()` - List recent workflow runs
  - `trigger_workflow()` - Dispatch workflow_dispatch event
  - `validate_workflow_sync()` - Compare local vs remote workflow files
  - `find_git_root()`, `get_current_branch()` - Git helpers
- `src/recompose/cli.py` - Added `--remote`, `--status`, `--force`, `--ref` to flow commands
- `tests/test_github.py` - 16 tests for GitHub CLI wrapper

See: `proj/P09_workflow_dispatch_DONE.md` for original plan

---

**Tree-based output formatting** - COMPLETE

Improved CLI output for flow execution with tree-based visual structure:

```
ci
│
├─▶ 1_gha.checkout
│     ✓ succeeded in 0.09s
│
├─▶ 4_lint
│     Running ruff check...
│     │ All checks passed!
│     Ruff check passed!
│     ✓ succeeded in 0.52s
│
⏹ Completed in 2.01s
```

Key implementation:
- **FlowRenderer** (`src/recompose/output.py`): Handles tree structure, headers, footers
- **TreePrefixWriter**: Wraps `sys.stdout`/`sys.stderr` to add tree prefixes to all output
- **TreeOutputContext**: Context manager that installs/restores wrapped streams
- **Subprocess nested indicators**: Subprocess output prefixed with dimmed `│` (stdout) or `!` (stderr)
- **Continuous vertical line**: Line extends from flow name to final `⏹` symbol
- **Skipped steps shown**: After failure, remaining steps shown as "⏭ skipped: prior failure in X"
- **Condition step formatting**: Special cyan color and expression display for eval_condition steps
- **Logging integration**: Updates logging handlers to use wrapped streams

Environment variables passed to subprocesses:
- `RECOMPOSE_TREE_MODE=1` enables tree output
- `RECOMPOSE_TREE_PREFIX` contains the prefix to use (e.g., `│    `)
- `RECOMPOSE_STEP_INDEX` / `RECOMPOSE_TOTAL_STEPS` for step context

---

**run_if conditional execution** - COMPLETE

Implemented `run_if()` context manager for conditional task execution within flows:

```python
@recompose.flow
def conditional_pipeline(*, run_extra: bool = False) -> None:
    setup.flow()

    with recompose.run_if(run_extra):  # Only runs if run_extra is True
        extra_validation.flow()

    finalize.flow()
```

Key implementation:
- **Expression algebra** (`src/recompose/expr.py`): Captures conditions without evaluating them
  - `InputExpr` for flow parameters
  - `BinaryExpr` for `==`, `!=`, `and`, `or` operators
  - `UnaryExpr` for `not`
- **Condition context** (`src/recompose/conditional.py`): `run_if()` context manager
- **Condition check steps**: Pseudo-tasks injected into flow plans that evaluate conditions
- **GHA integration**: Conditional steps get `if:` clause referencing condition-check output
- **Local execution**: `run_isolated` checks workspace for condition results, skips steps when false

Enforcement: `InputPlaceholder.__bool__()` raises `TypeError` to prevent direct use in Python `if` statements (flows must have static graphs).

Added `conditional_pipeline` example to `examples/tutorial/intro_flows.py` demonstrating the feature.

---

**P08_ci_integration** - COMPLETE.

Successfully got CI workflow running on GitHub Actions! The pipeline now:
1. ✅ GHA setup (checkout, python, uv) - via `recompose.gha.checkout.flow()` etc.
2. ✅ lint - ruff + mypy (required consolidating dev deps in `[dependency-groups]`)
3. ✅ format_check - ruff format --check
4. ✅ test - pytest (all 144 tests)
5. ✅ generate_gha --check_only - validates workflow consistency

Key implementation details for GHA:
- Added `python_cmd` config (e.g., "uv run python") via `recompose.main(python_cmd=...)`
- Added `working_directory` config for job-level `defaults.run.working-directory`
- Auto-detect module vs script invocation via `__spec__` - generates `-m module` style commands
- Script paths automatically adjusted when working_directory is set

Run `#20230043377` passed all core checks (lint, format, test). The generate_gha step
correctly detected our temporary push trigger as out of sync (expected - validates consistency).

---

**P07_real_examples** - COMPLETE (P07a, P07b, P07c, P07d done).

Created a **real, working CI/dev workflow** for the recompose project itself.
All tasks we use day-to-day. Proves the system works end-to-end.

What was delivered:
- `recompose/run` - THE canonical entrypoint
- Real tasks: lint, format_check, format, test, build_wheel, test_installed, etc.
- Real flows: `ci` flow composes tasks for CI
- Generated workflow: `.github/workflows/recompose_ci.yml`
- Validation: `validate_workflows` task for CI drift detection

## Phase breakdown:

**P07a - Examples structure & basic tasks** ✅ DONE
Restructured examples to be both instructional AND real:

```
recompose/
├── run                          # THE entry point: ./run lint, ./run ci
└── examples/
    ├── __init__.py              # Makes examples a proper package
    ├── README.md                # Concept introduction + walkthrough
    ├── app.py                   # Unified entrypoint (imports all)
    ├── tutorial/                # Incremental tutorials
    │   ├── intro_tasks.py       # Basic tasks, Results, subprocess
    │   ├── intro_taskclass.py   # @taskclass
    │   └── intro_flows.py       # Flows (imports from intro_tasks.py)
    ├── tasks/                   # Real tasks for recompose
    │   ├── __init__.py
    │   ├── lint.py              # lint, format_check, format
    │   └── test.py              # test
    └── flows/                   # Real flows
        ├── __init__.py
        └── ci.py                # ci flow (lint + format_check + test)
```

Key implementation details:
- `./run` wrapper script uses `python -m examples.app` for proper package imports
- Added entry point detection (`__spec__`) to preserve module invocation across subprocess calls
- `run_isolated` now uses `-m module` when entry point was invoked as module
- All examples use clean relative imports (no sys.path hackery)

**P07b - Build & distribution tasks** ✅ DONE
Added `examples/tasks/build.py` with:
- `build_wheel` - creates wheel with `uv build`
- `create_test_venv` - creates isolated venv for testing
- `install_wheel` - installs wheel + pytest into test venv
- `smoke_test` - quick validation using `examples/tasks/smoke_test.py`
- `test_installed` - runs full pytest suite against installed package

Key insight: using `{venv}/bin/python -m pytest {project}/tests/` ensures
tests run against the installed package (not source). All 144 tests pass.

**P07c - Unified entrypoint** ✅ DONE (merged into P07a)
Already completed as part of P07a:
- `./run` wrapper script is THE canonical entrypoint
- Usage: `./run lint`, `./run test`, `./run ci`, `./run inspect ci`

**P07d - Workflow generation & validation** ✅ DONE
Consolidated into built-in `generate_gha` task (in `src/recompose/builtin_tasks.py`):
- `./run generate_gha` - regenerates workflow files (default: all flows/automations)
- `./run generate_gha --check_only` - validates generated == committed (for CI)
- Defaults to `.github/workflows/` in git root
- Named as `recompose_flow_<name>.yml` and `recompose_automation_<name>.yml`

Updated `examples/flows/ci.py` to include GHA setup actions:
- `recompose.gha.checkout.flow()` - checkout repository
- `recompose.gha.setup_python(version="3.12").flow()` - setup Python
- `recompose.gha.setup_uv().flow()` - setup uv

Generated workflow written to `.github/workflows/recompose_flow_ci.yml` with header
identifying it as generated and instructions for modification.

# UPCOMING

(P09 moved to NOW)

# DEFERRED

**P05c_flows_parallel** - Parallel task execution within flows
- Currently flows execute tasks sequentially
- This matches the GHA step model (steps are sequential within a job)
- Parallel execution would be nice but adds complexity
- Defer until we have a clear use case that needs it

**Logging integration** - Replace `recompose.out` with Python logging
- Question: Does `recompose.out` need to exist at all?
- Could hook into Python's logging framework directly
- Task runner would set up logging infrastructure automatically
- Benefits:
  - Standard Python logging patterns
  - Automatic capture to recompose logs folder
  - Debug info available for inspection
  - Third-party library logs captured too
- Defer until we have more real usage to inform the design
- Current `recompose.out` works fine for now

**Result type serialization protocol** - COMPLETE (simpler approach via Pydantic)
- Problem solved: Types are now properly preserved through serialization
- Implementation uses Pydantic's `TypeAdapter` instead of custom protocol:
  - Serialization: Wrap complex types with `__type__` key storing `module.ClassName`
  - Deserialization: Resolve type, use `TypeAdapter.validate_python()` for reconstruction
- Handles automatically:
  - `Path` objects (serialized as strings, restored as Path)
  - Pydantic models (via `model_dump()` / TypeAdapter)
  - Dataclasses with nested structures (TypeAdapter handles all nesting)
- No explicit protocol needed - Pydantic handles type coercion
- ~60 lines simpler than manual `get_type_hints()` approach

# COMPLETED

- **P06_gha_generation** - All 3 phases complete. See `proj/P06_gha_generation_DONE.md`
  - Phase 1: Basic YAML generation (`generate-gha` CLI, workflow_dispatch inputs, actionlint)
  - Phase 2: GHA setup actions (`GHAAction` class, checkout/setup_python/setup_uv/setup_rust/cache)
  - Phase 3: Automations (`@automation` decorator, `.dispatch()` method, workflow_run orchestration)
- **P05d_flows_subprocess** - Subprocess isolation for flow tasks
- **P05b_declarative_flows** - Declarative flow execution with `.flow()` API
- **P05a_flows** - Sequential flows with `@flow` decorator
- **P04_member_tasks** - Class-based tasks via `@taskclass`
- **P03_subprocess** - Subprocess helpers: `recompose.run()`, `RunResult`
- **P02_cli** - CLI generation with Click
- **P01_foundation** - Core package with `@task`, `Result`, `Ok`/`Err`, `out`/`dbg`

# ARCHITECTURE DECISIONS

## Library Choices

| Need | Choice | Rationale |
|------|--------|-----------|
| CLI generation | **Click** | Mature, well-documented. We want control over CLI generation. |
| Result/data types | **Pydantic** | Perfect for typed, validatable Result classes. |
| Console output | **Rich** | Great for formatted output, progress bars, etc. |
| Async (later) | **asyncio** | Built-in. Only needed for parallel flow execution. |

## Design Principles

1. **Tasks are just functions** - The `@task` decorator minimally alters the function
2. **CLI is opt-in** - `recompose.main()` builds CLI, but tasks work without it
3. **Result is explicit** - Tasks return `Result[T]` with value + status + output
4. **Context is ambient** - Helpers detect if running inside recompose engine

## Hierarchy

- **Task** - Single unit of work (Python function with @task)
- **Flow** - Composition of tasks → Single GHA job, workflow_dispatch triggerable
- **Automation** - Orchestrates flows → Uses workflow_run to chain workflows

## Workflow Generation

- Workflows sync to top-level `.github/workflows/` directory
- Named as `recompose_flow_<name>.yml` and `recompose_automation_<name>.yml`
- Generated files include header comment identifying them as generated
- CI validates that committed workflows match what generator produces
- Single built-in task handles both: `generate_gha` (regen) / `generate_gha --check_only` (validate)

## Local-only vs CI Tasks

Some tasks are meant for local development only:
- `format` (apply fixes) - modifies files, not appropriate for CI
- `update_workflows` - regenerates workflow files

Some tasks run in CI:
- `lint`, `format_check`, `test` - validation tasks
- `validate_workflows` - ensures no manual workflow edits

# NOTES

- Keep it simple. Don't over-engineer early.
- Write tests as we go.
- Commit frequently with clear messages.
- Each sub-project should be usable independently before moving on.
