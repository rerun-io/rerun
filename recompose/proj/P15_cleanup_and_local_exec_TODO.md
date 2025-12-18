# P15: Cleanup & Local Automation Execution

## Overview

Follow-up refinements to P14's automation model plus a significant new feature: local automation execution.

## Issues to Address

### Issue 1: Unify dispatchables and automations

**Problem:** `dispatchables` and `automations` are conceptually the same thing. A dispatchable is just an automation with a `workflow_dispatch` trigger. Separate arguments create confusion.

**Solution:**
- Remove `dispatchables` parameter from `App`
- An automation that only has `workflow_dispatch` trigger IS a dispatchable
- `make_dispatchable(task)` creates a simple automation wrapping a single task
- Automations discover their dependencies via job analysis, not separate registration

### Issue 2: make_dispatchable should auto-infer inputs

**Problem:** Current API requires explicit inputs:
```python
test_workflow = recompose.make_dispatchable(
    test,
    inputs={
        "verbose": recompose.BoolInput(default=False, description="..."),
        "coverage": recompose.BoolInput(default=False, description="..."),
    },
)
```

**Solution:** Task already declares its parameters. `make_dispatchable(test)` should:
1. Inspect task signature
2. Infer input types from annotations (str → StringInput, bool → BoolInput)
3. Use parameter defaults for input defaults
4. Use docstring/annotations for descriptions if available

Explicit inputs= only needed when you want to customize (different description, choices, etc.)

### Issue 3: Rename python_cmd to cli_command

**Problem:** `python_cmd="uv run python"` is awkward. Generated workflows use `./run lint`, not `uv run python -m examples.app lint`.

**Solution:**
- Rename to `cli_command="./run"` (or `entry_point`)
- This is what gets used in generated workflow steps
- Remove module_name tracking since we're not using it for subprocess invocation anymore

### Issue 4: Local Automation Execution (BIG)

**Problem:** Can't test automations locally. Want to verify:
- Dependency analysis is correct
- Tasks can be invoked with correct args
- Env vars and inputs flow properly

**Solution:** Add `./run <automation-name>` support that:
1. Parses the automation to get job graph
2. Executes jobs as subprocesses in dependency order
3. Passes outputs between jobs (via temp files or stdout capture)
4. Handles InputParam values from CLI args
5. Skips GHA-specific setup steps (checkout, setup-python, setup-uv)
6. Reports results in a nice format

**Limitations (acceptable):**
- Can't test matrix jobs (would need to expand and run each combination)
- Can't test artifact upload/download (just skip those steps)
- Can't test secrets (require them to be in env or local config)

---

## Implementation Plan

### Phase 1: API Cleanup (Issues 1-3)

1. **Update make_dispatchable() for auto-inference**
   - Inspect task signature
   - Map types to input types
   - Use defaults from signature
   - Allow explicit inputs to override

2. **Update App class**
   - Remove `dispatchables` parameter
   - Rename `python_cmd` to `cli_command` (default: `"./run"`)
   - Remove `_module_name` tracking if no longer needed

3. **Update generate_gha**
   - Auto-discover dispatchables from automations
   - Use `cli_command` directly
   - Simplify entry point logic

4. **Update examples**
   - Simplify app.py

**Tests:** Update existing tests, verify generation still works

### Phase 2: Local Automation Execution (Issue 4)

1. **Add automation execution to CLI**
   - When user runs `./run <automation-name>`, execute locally
   - Parse automation to get job list
   - Build execution plan from dependencies

2. **Implement LocalExecutor**
   - Execute jobs sequentially respecting `needs:`
   - Invoke tasks via subprocess: `./run <task-name> --arg=value`
   - Capture outputs for passing to dependent jobs
   - Handle InputParam values from CLI args

3. **Output/Artifact passing**
   - Job outputs written to temp files (like GITHUB_OUTPUT)
   - Dependent jobs read from those temp files
   - Artifact paths passed as-is (local files)

4. **Nice reporting**
   - Show job progress
   - Show which jobs passed/failed
   - Show total time

**Tests:**
- Test simple automation (no deps)
- Test automation with dependencies
- Test automation with inputs
- Test failing job stops execution

---

## Completion Criteria

### Phase 1
- [ ] `make_dispatchable(task)` works without explicit inputs
- [ ] `App(cli_command="./run")` replaces `python_cmd`
- [ ] `dispatchables=` removed from App
- [ ] Examples simplified
- [ ] All tests pass

### Phase 2
- [ ] `./run ci` executes the ci automation locally
- [ ] Jobs run in correct dependency order
- [ ] Outputs pass between jobs
- [ ] InputParams become CLI arguments
- [ ] Nice progress/result reporting
- [ ] All tests pass

---

## Design Notes

### Local execution model

```
./run ci                     # Run ci automation locally
./run ci --dry-run          # Show what would run without executing
./run ci --skip-tests=true  # Pass InputParam values
```

Internally:
1. Parse automation → list of JobSpec with dependencies
2. Topological sort by `needs:`
3. For each job:
   - Set up temp GITHUB_OUTPUT file
   - Run: `./run <task-name> --arg1=$val1 --arg2=$val2`
   - Capture outputs from temp file
   - Store for dependent jobs

### Output passing

Jobs declare outputs via `@task(outputs=["wheel_path"])`.
When running locally:
- Task writes to GITHUB_OUTPUT (recompose already does this)
- Executor reads the file after task completes
- Passes values to dependent jobs via CLI args

### Skipped steps

When running locally, skip:
- checkout (already have code)
- setup-python, setup-uv (assume env is set up)
- upload-artifact, download-artifact (use local paths)

These are just no-ops in local mode.
