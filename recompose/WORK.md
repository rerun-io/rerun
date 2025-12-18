# NOW

**P15: Cleanup & Local Automation Execution** - Phase 1 COMPLETE

See `proj/P15_cleanup_and_local_exec_TODO.md` for full plan.

## Phase 1 COMPLETE: API Cleanup

Simplified and unified the dispatchable/automation API:

- **Renamed `python_cmd` to `cli_command`** (default: `"./run"`)
  - Updated `App` class, `context.py`, `cli.py`
  - Better reflects actual usage (entry point, not Python command)

- **Unified dispatchables into automations**
  - `make_dispatchable(task)` now returns `AutomationWrapper` (was `Dispatchable`)
  - Removed `dispatchables=` parameter from `App` - use `automations=` instead
  - Dispatchables are just automations with `workflow_dispatch` trigger
  - Removed `get_dispatchables()` from context (no longer needed)
  - `generate_gha` only uses `render_automation_jobs()` now

- **Simplified workflow naming**
  - All workflows named `recompose_<name>.yml` (was split by type)

**Example migration:**
```python
# Before:
app = App(
    python_cmd="uv run python",
    automations=[ci],
    dispatchables=[lint_workflow, test_workflow],
)

# After:
app = App(
    cli_command="./run",
    automations=[ci, lint_workflow, test_workflow],
)
```

**Test results:** 209 tests pass, ruff clean

## Next: Phase 2 - Local Automation Execution

- Run automations locally: `./run ci`
- Execute jobs as subprocesses in dependency order
- Pass outputs between jobs via temp files
- Handle InputParams from CLI args
- Skip GHA-specific steps (checkout, setup-python)

---

**P14_architectural_pivot** - COMPLETE. All 7 phases done.

See `proj/P14_architectural_pivot_DONE.md` for full design.

**Key changes:**
- Tasks map to GHA **jobs** (not steps) - each job runs one task via CLI
- `@flow` removed - tasks calling tasks is just Python, no graph building
- `@taskclass` removed - no class-state sync across GHA jobs
- `@automation` orchestrates multi-job workflows with inferred `needs:`
- Artifacts, secrets, outputs, setup declared in `@task` decorator
- Generated workflow steps use app's entry_point (copy-paste runnable locally)

**Backup branch:** `jleibs/recompose-backup-flows-as-steps` preserves old approach.

## Phase 7 COMPLETE: Migration & Polish

Final cleanup and example migration:

- **builtin_tasks.py**: Updated `generate_gha` to use automations and dispatchables
  - Removed old flow references
  - Uses `render_automation_jobs()` for automations
  - Uses `render_dispatchable()` for dispatchables
  - Updated `inspect` to handle automations and dispatchables

- **App class**: Added `dispatchables` parameter for workflow generation

- **context.py**: Added `get_dispatchables()` function for registry access

- **Examples migrated**:
  - Deleted `examples/flows/` directory (old flow-based code)
  - Deleted `examples/tasks/virtual_env.py` (TaskClass, no longer supported)
  - Created `examples/automations/ci.py` with new `@automation` pattern
  - Updated `examples/app.py` with automations and dispatchables

**Test results:** 209 tests pass, ruff clean

## Phase 6 COMPLETE: Cleanup Old Code

Removed all legacy flow-based code:
- Deleted source files: `flow.py`, `plan.py`, `workspace.py`, `_run_step.py`, `local_executor.py`, `automation.py` (old), `conditional.py`, `expr.py`
- Cleaned `task.py`: removed `@taskclass`, `@method`, `TaskClassInfo`, `_TaskClassNodeProxy`, `_TaskMethodCaller`
- Cleaned `gha.py`: removed `render_flow_workflow`, `render_automation_workflow` (old), flow rendering functions
- Cleaned `context.py`: removed flow registry functions
- Cleaned `cli.py`: removed `_build_flow_command`, `FlowWrapper` references
- Cleaned `__init__.py`: removed legacy exports
- Deleted test files: `test_flow.py`, `test_declarative_flow.py`, `test_workspace.py`, `test_taskclass_flow.py`, `test_member_tasks.py`, `test_parameterized_flows.py`, `test_automation.py` (old), `flow_test_app.py`

**Test results:** 209 tests pass (down from 318 - removed 109 legacy tests)

## Phase 5 COMPLETE: make_dispatchable()

Implemented in `jobs.py` and `gha.py`:
- `DispatchInput` base class with `StringInput`, `BoolInput`, `ChoiceInput` subclasses
- `Dispatchable` class wrapping a task for workflow_dispatch triggering
- `DispatchableInfo` dataclass for dispatchable metadata
- `make_dispatchable(task, inputs=None, name=None)` function
- `render_dispatchable()` function in gha.py

## Phase 4 COMPLETE: Workflow Generation

Implemented in `gha.py`:
- `render_automation_jobs(automation, entry_point, default_setup, working_directory)` - Main function
- `GHAJobSpec` class with support for needs, outputs, if_condition, matrix
- `SetupStep` class for configuring setup steps
- `DEFAULT_SETUP_STEPS` - checkout, setup-python, setup-uv

## Phase 2+3 COMPLETE: Automation Framework & Triggers

Implemented in `jobs.py`:
- `@automation` decorator with context tracking
- `job()` function returning `JobSpec`
- Job output/artifact references with automatic dependency inference
- `InputParam[T]` type for automation parameters
- Condition expressions with `&`, `|`, `~`, `==`, `!=`
- `github.*` context references
- Trigger types (on_push, on_pull_request, on_schedule, on_workflow_dispatch)

## Phase 1 COMPLETE: Task Decorator Enhancements

Implemented:
- `@task(outputs=["..."], artifacts=["..."], secrets=["..."], setup=[...])` decorator parameters
- `set_output(name, value)` - validates against declared outputs, writes to GITHUB_OUTPUT
- `save_artifact(name, path)` - validates against declared artifacts
- `get_secret(name)` - validates against declared secrets
- `step(name)` context manager and `@step_decorator` for visual output grouping

# UPCOMING

**P15: Cleanup & Local Automation Execution**

See `proj/P15_cleanup_and_local_exec_TODO.md` for full plan.

Two parts:

**Phase 1: API Cleanup**
- Unify dispatchables into automations (remove `dispatchables=` from App)
- `make_dispatchable(task)` auto-infers inputs from task signature
- Rename `python_cmd` to `cli_command` (default: `"./run"`)

**Phase 2: Local Automation Execution** (bigger)
- Run automations locally: `./run ci`
- Execute jobs as subprocesses in dependency order
- Pass outputs between jobs via temp files
- Handle InputParams from CLI args
- Skip GHA-specific steps (checkout, setup-python)

# DEFERRED

(Empty - no deferred items)

# RECENTLY COMPLETED

- P14 Phase 7: Migration & Polish - examples migrated, generate_gha updated
- P14 Phase 6: Cleanup old code (flow, taskclass, etc.)
- P14 Phases 1-5: Full P14 implementation

Previous work preserved in `jleibs/recompose-backup-flows-as-steps`:
- P01-P13: Foundation through TaskClass in flows
- Full flow-as-steps model with subprocess isolation
- GHA generation for flows → single-job workflows

# ARCHITECTURE DECISIONS

## New Hierarchy (P14)

- **Task** - Single unit of work, maps to one GHA job
- **Automation** - Orchestrates tasks as multi-job workflow with `needs:`
- **Dispatchable** - Wrapper to make a task workflow_dispatch triggerable

## Key Principles

1. **What you see is what you run** - Generated steps use actual CLI commands
2. **Explicit over magic** - `.job()` calls explicit, dependencies from references
3. **Validate at construction** - Automations validate during decoration
4. **String outputs for GHA** - Embrace GitHub's string-based job outputs

## Task Decorator Parameters

| Parameter | Purpose |
|-----------|---------|
| `outputs` | String outputs (to GITHUB_OUTPUT) |
| `artifacts` | File artifacts (upload/download) |
| `secrets` | Required secrets (from GHA or local file) |
| `setup` | Override default GHA setup steps |

## Automation/Job Types (Phase 2)

| Type | Purpose |
|------|---------|
| `JobSpec` | Represents a job in an automation |
| `JobOutputRef` | Reference to a job's output (creates dependency) |
| `ArtifactRef` | Reference to a job's artifact (creates dependency) |
| `InputParam[T]` | Automation input parameter (→ workflow_dispatch input) |
| `Artifact` | Type hint for artifact inputs to tasks |
| `ConditionExpr` | Job condition expression |
| `Trigger` | Workflow trigger (on_push, etc.) |

## Workflow Generation (Phase 4)

| Component | Purpose |
|-----------|---------|
| `render_automation_jobs()` | Main function to generate WorkflowSpec from automation |
| `render_dispatchable()` | Generate WorkflowSpec from dispatchable |
| `GHAJobSpec` | Represents a GHA job with needs, outputs, if, matrix |
| `SetupStep` | Represents a setup step (checkout, setup-python, etc.) |
| `DEFAULT_SETUP_STEPS` | Default setup: checkout + python 3.12 + uv |
