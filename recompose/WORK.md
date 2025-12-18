# NOW

**P14_architectural_pivot** - Major redesign: Tasks as Jobs, not Steps.

See `proj/P14_architectural_pivot_DONE.md` for full design.

**Key changes:**
- Tasks map to GHA **jobs** (not steps) - each job runs one task via CLI
- `@flow` removed - tasks calling tasks is just Python, no graph building
- `@taskclass` removed - no class-state sync across GHA jobs
- `@automation` orchestrates multi-job workflows with inferred `needs:`
- Artifacts, secrets, outputs, setup declared in `@task` decorator
- Generated workflow steps use app's entry_point (copy-paste runnable locally)

**Backup branch:** `jleibs/recompose-backup-flows-as-steps` preserves old approach.

**Current phase:** Phase 6 COMPLETE. All legacy code removed.

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

1. **Phase 7: Migration & Polish** (NEXT)
   - Migrate examples to new model
   - Update App class for dispatchables parameter
   - Update builtin_tasks.generate_gha to handle automations and dispatchables
   - Documentation

# DEFERRED

(Empty - no deferred items)

# RECENTLY COMPLETED

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
