# Recompose Architecture

## Overview

Recompose is a lightweight, typed, Pythonic task execution framework. It provides:
- **Tasks**: Single units of work (Python functions with `@task`)
- **Automations**: Multi-job workflows that orchestrate tasks
- **GHA Integration**: Automatic generation of GitHub Actions workflows

## Module Organization

```
src/recompose/
├── __init__.py           # Public API exports
│
├── # Core Abstractions
├── task.py               # @task decorator, TaskInfo, TaskWrapper
├── jobs.py               # @automation decorator, JobSpec, triggers, conditions
├── result.py             # Result[T], Ok(), Err() - task return types
│
├── # Execution
├── context.py            # Ambient context: registries, debug mode, output helpers
├── local_executor.py     # Local execution of automations
├── subprocess.py         # run() helper for spawning child processes
├── step.py               # step() context manager for grouping output
│
├── # Output
├── output.py             # Tree-based output rendering (OutputManager)
│
├── # CLI
├── cli.py                # Click-based CLI generation from tasks/automations
├── command_group.py      # App, CommandGroup configuration
├── builtin_tasks.py      # generate_gha, inspect built-in commands
│
├── # GitHub Actions
├── gha.py                # Workflow YAML generation (WorkflowSpec, GHAJobSpec, etc.)
├── gh_cli.py             # gh CLI wrapper (trigger workflows, check status)
```

## Core Concepts

### Tasks (`task.py`)

A **task** is a Python function decorated with `@task`. Tasks:
- Return `Result[T]` to indicate success/failure with typed values
- Automatically catch exceptions and convert to `Err` results
- Map to GHA **jobs** (one task = one job)

Key types:
- `TaskInfo`: Metadata (name, signature, outputs, artifacts, secrets, setup)
- `TaskWrapper[P, T]`: Protocol for decorated task functions

### Automations (`jobs.py`)

An **automation** orchestrates multiple tasks as a multi-job GHA workflow:
- Tasks are added via `job()` calls that return `JobSpec`
- Dependencies are inferred from `JobOutputRef` and `ArtifactRef` usage
- Triggers (push, PR, schedule, workflow_dispatch) define when to run

Key types:
- `AutomationInfo`: Metadata about an automation
- `JobSpec`: Represents a job in an automation
- `JobOutputRef`: Reference to a job's string output (creates dependency)
- `ArtifactRef`: Reference to a job's artifact (creates dependency)
- `InputParam[T]`: Automation input parameter (→ workflow_dispatch input)

### Results (`result.py`)

All tasks return `Result[T]`:
- `Ok(value)`: Success with a typed value
- `Err(message)`: Failure with error message

### Context (`context.py`)

The ambient context provides:
- **Task/automation registries**: Populated by `App.main()`
- **Output helpers**: `out()`, `dbg()` for task output
- **Task outputs**: `set_output()`, `save_artifact()`, `get_secret()`
- **CLI command**: For GHA workflow generation (e.g., "./run")
- **Working directory**: For GHA workflows

### Local Execution (`local_executor.py`)

`LocalExecutor` runs automations locally:
- Jobs execute as subprocesses via CLI (`./run task-name --arg=value`)
- Dependencies respected via topological sort
- Outputs passed between jobs via temp files

## Data Flow

### Local Execution (Direct Call)
```
User calls task() → task wrapper executes function → returns Result[T]
```

### Local Execution (Automation)
```
User calls ./run automation-name →
  1. Build job graph from automation
  2. Topological sort jobs by dependencies
  3. Execute each job as subprocess via CLI
  4. Pass outputs between jobs via temp files
  → returns AutomationResult
```

### GHA Generation
```
generate_gha →
  1. For each automation: render_automation_jobs() → WorkflowSpec
  2. Each job() call → GHAJobSpec with needs, outputs, artifacts
  3. Write YAML to .github/workflows/
```

## Design Principles

1. **Tasks are just functions**: Minimal decoration, callable as normal Python
2. **What you see is what you run**: Generated steps use actual CLI commands
3. **Explicit over magic**: `job()` calls explicit, dependencies from references
4. **Result is explicit**: Tasks return `Result[T]` with value + status
5. **Local/CI parity**: Automations execute identically locally and in GHA
