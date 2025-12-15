# NOW

**P08_ci_integration** - Prove it works in real CI.

Next step: Push branch to GitHub and verify the generated workflow actually runs.
This is the end-to-end validation that the whole system works.

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
Added `examples/tasks/workflows.py` with:
- `update_workflows` - regenerates workflow files (local dev task)
- `validate_workflows` - checks generated == committed (CI task)

Updated `examples/flows/ci.py` to include GHA setup actions:
- `recompose.gha.checkout.flow()` - checkout repository
- `recompose.gha.setup_python(version="3.12").flow()` - setup Python
- `recompose.gha.setup_uv().flow()` - setup uv

Generated workflow written to `.github/workflows/recompose_ci.yml` with header
identifying it as generated and instructions for modification.

# UPCOMING

**P09_workflow_dispatch** - Ergonomic CLI-to-GitHub integration
- Use recompose's knowledge of flows to find corresponding workflow runs on GitHub
- Add flag to kick off a flow on GitHub instead of running locally
  - e.g., `./run ci --remote` triggers the workflow on GitHub
- Before dispatch, validate that workflow file on GitHub matches local state
- Produce warning/error if workflow is out of sync (prevents running stale workflows)
- Bonus: show workflow run status, link to logs, etc.

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

**Result type serialization protocol** - Proper support for custom types in flows
- Current: workspace.py has basic serialization, loses type info on deserialize
- Problem: `Result[Path]` serializes to string, deserializes as string (not Path)
- Solution: Protocol-based type handling with two approaches:
  1. **Direct protocol**: Types implement `RecomposeSerializable` protocol
     - `def __recompose_serialize__(self) -> dict`
     - `@classmethod def __recompose_deserialize__(cls, data: dict) -> Self`
  2. **Registered helpers**: External types register a helper class
     - `recompose.register_type(Path, PathSerializer)`
     - Helper handles ser/deser for types you don't control
- Recompose registers built-in helpers for: `Path`, `datetime`, etc.
- Recompose extension types (e.g., `Artifact`) implement protocol directly
- Pydantic BaseModel subclasses work automatically via `.model_dump()` / `.model_validate()`
- For now: use strings for paths in P07b, revisit when patterns are clearer

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
- Generated files include header comment identifying them as generated
- CI validates that committed workflows match what generator produces
- Local `update_workflows` task regenerates; CI `validate_workflows` task checks

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
