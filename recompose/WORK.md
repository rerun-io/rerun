# NOW

No active project - ready for next task.

---

**P16: Unified Task Output System** - COMPLETE

See `proj/P16_unified_output_DONE.md` for full details.

## Summary

Created unified `OutputManager` class for all output formatting:
- Tree-style hierarchy for nested tasks and automations
- Rich console colors and consistent styling
- GHA detection with `::group::` markers
- Proper symbols: `▼`, `├─▶`, `└─▶`, `⊕─┬─▶`, `✓`, `✗`
- Recursive capture-and-prefix model: each task captures child output and applies prefixes
- Color styling separated: cyan for tree chars, green for ✓, red for ✗

**Files modified:**
- `src/recompose/output.py` - OutputManager, prefix_task_output, print_task_output_styled
- `src/recompose/task.py` - Uses recursive capture model for nested tasks
- `src/recompose/local_executor.py` - Uses OutputManager for automations

**Test results:** 234 tests pass, ruff clean

---

**P15: Cleanup & Local Automation Execution** - COMPLETE

See `proj/P15_cleanup_and_local_exec_DONE.md` for full plan.

## Phase 2 COMPLETE: Local Automation Execution

Implemented local execution of automations - run `./run ci` to execute automations locally:

- **LocalExecutor** (`local_executor.py`):
  - `LocalExecutor` class for executing automations as subprocesses
  - `topological_sort()` for ordering jobs by dependencies
  - `_execute_job()` runs tasks via CLI (`./run task-name --arg=value`)
  - Job outputs captured via temp GITHUB_OUTPUT files
  - Output passing between jobs via resolved `JobOutputRef`

- **CLI Integration** (`cli.py`):
  - `_build_automation_command()` creates Click commands for automations
  - `_add_automation_to_cli()` registers automations in "Automations" group
  - Automations appear in `./run --help` under "Automations" section
  - Common options: `--dry-run`, `--verbose`
  - InputParam values become CLI arguments

- **New exports** (`__init__.py`):
  - `LocalExecutor` - class for programmatic execution
  - `execute_automation()` - convenience function
  - `AutomationResult`, `JobResult` - result types

**Usage:**
```bash
# Run automation locally
./run ci

# Dry run (show what would execute)
./run ci --dry-run

# Verbose output
./run ci --verbose

# With input parameters (for automations with InputParam)
./run deploy --environment=prod
```

**Test results:** 234 tests pass (25 new), ruff clean

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

# UPCOMING

(Determine next priorities based on project needs)

# DEFERRED

(Empty - no deferred items)

# RECENTLY COMPLETED

- P16: Unified Task Output System - colored tree output for tasks and automations
- P15 Phase 2: Local Automation Execution - `./run ci` now works
- P15 Phase 1: API Cleanup - unified dispatchables/automations, renamed cli_command
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
