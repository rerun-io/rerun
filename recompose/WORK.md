# NOW

**P14_architectural_pivot** - Major redesign: Tasks as Jobs, not Steps.

See `proj/P14_architectural_pivot_TODO.md` for full design.

**Key changes:**
- Tasks map to GHA **jobs** (not steps) - each job runs one task via CLI
- `@flow` removed - tasks calling tasks is just Python, no graph building
- `@taskclass` removed - no class-state sync across GHA jobs
- `@automation` orchestrates multi-job workflows with inferred `needs:`
- Artifacts, secrets, outputs, setup declared in `@task` decorator
- Generated workflow steps use app's entry_point (copy-paste runnable locally)

**Backup branch:** `jleibs/recompose-backup-flows-as-steps` preserves old approach.

**Current phase:** Phase 2+3 COMPLETE. Ready for Phase 4 (Workflow Generation).

## Phase 2 COMPLETE: Automation Framework

Implemented in `jobs.py`:
- `@automation` decorator with context tracking
- `job()` function returning `JobSpec`
- `JobSpec.get()` returning `JobOutputRef` for output references
- `JobSpec.artifact()` returning `ArtifactRef` for artifact references
- Dependency inference from `JobOutputRef`/`ArtifactRef` in job inputs
- `InputParam[T]` type for automation parameters
- `Artifact` type for artifact inputs to tasks
- Condition expression algebra (`&`, `|`, `~`, `==`, `!=`)
- `github.*` context references for conditions (ref_name, event_name, etc.)
- Trigger types (on_push, on_pull_request, on_schedule, on_workflow_dispatch)
- 47 new tests, all passing (266 total)

## Phase 1 COMPLETE: Task Decorator Enhancements

Implemented:
- `@task(outputs=["..."], artifacts=["..."], secrets=["..."], setup=[...])` decorator parameters
- `set_output(name, value)` - validates against declared outputs, writes to GITHUB_OUTPUT
- `save_artifact(name, path)` - validates against declared artifacts
- `get_secret(name)` - validates against declared secrets, reads from env or ~/.recompose/secrets.toml
- `Result.outputs` and `Result.artifacts` properties
- `step(name)` context manager and `@step_decorator` for visual output grouping
- 24 tests for Phase 1

## Phase 3 COMPLETE: Triggers (implemented in Phase 2)

Implemented:
- `on_push(branches=[], tags=[], paths=[])` trigger
- `on_pull_request(branches=[], types=[], paths=[])` trigger
- `on_schedule(cron=...)` trigger
- `on_workflow_dispatch()` trigger
- Trigger combination with `|` operator
- All triggers have `to_gha_dict()` for YAML generation

# UPCOMING

1. **Phase 4: Workflow Generation** (NEXT)
   - Update GHA generation for new multi-job model
   - Generate jobs using app's entry_point
   - Handle job outputs/inputs mapping
   - Handle artifact upload/download steps
   - Handle secrets in job env
   - Handle per-task setup overrides
   - Handle matrix jobs

2. Phase 5: make_dispatchable() for single-task workflows
3. Phase 6-7: Cleanup old code, migration, documentation

# DEFERRED

(Empty - no deferred items)

# RECENTLY COMPLETED

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
