# P05d: Subprocess Isolation for Flow Tasks

**Status:** DONE

## Goal

Enable flow tasks to run as separate subprocess invocations, with results serialized
to JSON files in a workspace directory. This is the foundation for GHA workflow generation,
where each step is a separate process invocation.

## Implementation

### Step Names

Tasks in a flow are assigned sequential step names like `1_fetch_source`, `2_compile_source`.
This ensures unique names even when the same task is used multiple times.

```python
plan = my_flow.plan()
plan.assign_step_names()

for step_name, node in plan.get_steps():
    print(f"{step_name}: {node.task_info.name}")
```

### Workspace

A workspace is a directory containing:
- `_params.json` - Flow name, parameters, step names, script path
- `{step_name}.json` - Result from each step (status, value, error, traceback)

```python
from recompose.workspace import (
    create_workspace,
    write_params,
    read_params,
    write_step_result,
    read_step_result,
)
```

### CLI Options

Flows get `--setup`, `--step`, and `--workspace` options automatically:

```bash
# Initialize workspace with parameters
./app.py build_pipeline --setup --workspace /tmp/ws --repo main

# Execute a single step
./app.py build_pipeline --step 1_fetch_source --workspace /tmp/ws

# Steps can be referenced by number, full name, or task name
./app.py build_pipeline --step 1 --workspace /tmp/ws
./app.py build_pipeline --step fetch_source --workspace /tmp/ws
```

### run_isolated() Method

For local testing, `flow.run_isolated()` orchestrates subprocess execution:

```python
@recompose.flow
def build_pipeline(*, repo: str) -> None:
    source = fetch_source.flow(repo=repo)
    compile_source.flow(source_dir=source)

# Run with subprocess isolation (like GHA would)
result = build_pipeline.run_isolated(repo="test")
```

This:
1. Creates a workspace directory
2. Writes `_params.json` with flow params and step names
3. Executes each step as a subprocess: `python script.py flow --step X --workspace Y`
4. Each step reads its dependencies from workspace and writes its result
5. Returns success if all steps complete

## Files Changed

- `src/recompose/flowgraph.py` - Added `step_name` field, step assignment methods
- `src/recompose/workspace.py` - NEW: Workspace management, result serialization
- `src/recompose/cli.py` - Added `--setup`, `--step`, `--workspace` options
- `src/recompose/flow.py` - Added `run_isolated()` method
- `src/recompose/__init__.py` - Exported workspace types
- `tests/test_workspace.py` - NEW: 14 tests for workspace functionality

## Tests

101 tests passing:
- `test_workspace.py` - FlowParams serialization, workspace I/O, step names, run_isolated

## Design Decisions

1. **File-based serialization** - JSON files in workspace directory. Simple, debuggable,
   works across processes.

2. **Sequential step names** - `1_fetch`, `2_build` ensures uniqueness even with repeated tasks.

3. **Script path from module** - `inspect.getfile(fn)` gets the flow's module, not the caller.

4. **Params stored once** - Flow parameters written in setup, not passed to each step.
   Steps read dependencies from workspace.

## Next Steps

P06_gha_generation can now generate workflow YAML where each step is:
```yaml
- name: 1_fetch_source
  run: python script.py build_pipeline --step 1_fetch_source --workspace ${{ github.workspace }}/.recompose
```
