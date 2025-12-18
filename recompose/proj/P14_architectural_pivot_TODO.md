# P14: Architectural Pivot - Tasks as Jobs

## The Fundamental Shift

**Old model:** Flow = GHA Job, Task = GHA Step
- Each flow maps to one workflow with one job
- Each task in the flow becomes a step within that job
- Complex state serialization needed between steps
- Graph-building at decoration time with InputPlaceholders and TaskNodes

**New model:** Task = GHA Job, Automation = Multi-Job Workflow
- Each task can map to its own GHA job (with setup + run step)
- Automations orchestrate multiple tasks/jobs with inferred `needs:` dependencies
- Flows (hierarchical task composition) are just regular Python - no graph building
- Clean separation: local execution vs CI orchestration

## Core Design Principles

1. **What you see is what you run**: Generated workflow steps use the same CLI invocation a user would type locally
2. **Explicit over magic**: `.job()` calls are explicit; dependencies inferred from input references
3. **Validate at construction time**: Automations validate during decoration/construction
4. **String outputs for GHA**: Embrace GitHub's string-based job outputs

---

## Task Outputs

Tasks can declare outputs in the decorator and set them via a helper:

```python
@recompose.task(outputs=["wheel_path", "version"])
def build_wheel() -> Result[None]:
    """Build a wheel and output its path."""
    result = run("uv", "build", "--wheel")

    # Set outputs (validates against declared outputs)
    recompose.set_output("wheel_path", "/dist/pkg-1.0.0.whl")
    recompose.set_output("version", "1.0.0")

    return Ok(None)
```

- `recompose.set_output(name, value)` - Sets an output value
  - Raises error if `name` not declared in `@task(outputs=[...])`
  - Writes to `GITHUB_OUTPUT` when running in GHA
  - Stores in context for local access
- Outputs are available on the Result: `result.outputs["wheel_path"]`

---

## Artifacts

Tasks can produce artifacts (files to be shared between jobs or preserved):

```python
@recompose.task(artifacts=["wheel"])
def build_wheel() -> Result[None]:
    """Build a wheel."""
    result = run("uv", "build", "--wheel")
    wheel_path = Path("dist/pkg-1.0.0.whl")

    # Save artifact (validates against declared artifacts)
    recompose.save_artifact("wheel", wheel_path)

    return Ok(None)
```

- `artifacts=["name"]` in decorator declares artifact outputs
- `recompose.save_artifact(name, path)` - Saves artifact
  - Validates name against declared artifacts
  - In GHA: automation adds `actions/upload-artifact` step after task
  - Locally: records path for downstream tasks

### Artifact as Job Input

Artifacts can be inputs to downstream jobs:

```python
@recompose.task
def test_wheel(wheel: recompose.Artifact) -> Result[None]:
    """Test an installed wheel."""
    # wheel is a Path to the artifact
    run("pip", "install", str(wheel))
    return Ok(None)
```

In automation:
```python
@recompose.automation
def build_and_test() -> None:
    build_job = recompose.job(build_wheel)

    test_job = recompose.job(
        test_wheel,
        inputs={
            "wheel": build_job.artifact("wheel"),  # Returns ArtifactRef
        },
    )
```

**Generated GHA:**
- `build_wheel` job has `actions/upload-artifact` step after task
- `test_wheel` job has `actions/download-artifact` step before task
- Downloaded path passed as `--wheel=/path/to/artifact`

**Local CLI:**
```bash
./run test_wheel --wheel=/dist/pkg-1.0.0.whl
```

---

## Secrets

Tasks that need secrets must declare them in the decorator:

```python
@recompose.task(secrets=["PYPI_TOKEN", "AWS_ACCESS_KEY"])
def publish_wheel() -> Result[None]:
    """Publish wheel to PyPI."""
    token = recompose.get_secret("PYPI_TOKEN")
    # Use token...
    return Ok(None)
```

- `secrets=["NAME"]` in decorator declares required secrets
- `recompose.get_secret(name)` - Gets secret value
  - Validates name against declared secrets
  - In GHA: automation adds secret to job's env from `${{ secrets.NAME }}`
  - Locally: reads from `~/.recompose/secrets.toml` (scoped to declared secrets only)

**Local secrets file** (`~/.recompose/secrets.toml`):
```toml
PYPI_TOKEN = "pypi-xxx..."
AWS_ACCESS_KEY = "AKIA..."
AWS_SECRET_KEY = "..."
```

Tasks only see secrets they declared - prevents accidental secret leakage.

**Generated GHA:**
```yaml
jobs:
  publish_wheel:
    runs-on: ubuntu-latest
    env:
      PYPI_TOKEN: ${{ secrets.PYPI_TOKEN }}
      AWS_ACCESS_KEY: ${{ secrets.AWS_ACCESS_KEY }}
    steps:
      - ...
      - run: ./run publish_wheel
```

---

## Visual Step Grouping

For organizing output within a task, use `@step` decorator or `with recompose.step()` context manager:

```python
@recompose.task
def build_and_test() -> Result[None]:
    with recompose.step("Compile"):
        run("cargo", "build", "--release")

    with recompose.step("Run tests"):
        run("cargo", "test")

    with recompose.step("Package"):
        run("tar", "-czf", "dist.tar.gz", "target/release/binary")

    return Ok(None)
```

**Local output** (nested tree):
```
▶ build_and_test
  ├─ Compile
  │  └─ cargo build --release
  ├─ Run tests
  │  └─ cargo test
  └─ Package
     └─ tar -czf dist.tar.gz target/release/binary
✓ build_and_test (12.3s)
```

**GHA output** (flat groups):
```
::group::Compile
cargo build --release
::endgroup::
::group::Run tests
cargo test
::endgroup::
::group::Package
tar -czf dist.tar.gz target/release/binary
::endgroup::
```

Steps can also be used as a decorator for helper functions:

```python
@recompose.step("Compile")
def compile_code():
    run("cargo", "build", "--release")

@recompose.task
def build() -> Result[None]:
    compile_code()  # Output grouped under "Compile"
    return Ok(None)
```

---

## Setup Dependencies

Tasks can declare their setup requirements in the decorator:

```python
@recompose.task(
    setup=[
        recompose.setup_rust(toolchain="nightly"),
        recompose.setup_python("3.12"),
    ]
)
def build_rust_extension() -> Result[None]:
    """Build a Rust extension that needs both Rust and Python."""
    ...
```

- `setup=[...]` in decorator declares setup steps for this task
- Overrides app-level default setup when specified
- In GHA: job uses task's setup steps instead of defaults
- Locally: setup steps are no-ops (user's local env)

---

## Dispatchable Tasks

Simple one-liner to create a workflow-dispatchable version of a task:

```python
lint_workflow = recompose.make_dispatchable(lint)

# Or for tasks with parameters:
test_workflow = recompose.make_dispatchable(
    test,
    inputs={
        "verbose": recompose.BoolInput(default=False),
    },
)
```

This generates a single-job workflow that:
- Has workflow_dispatch trigger with specified inputs
- Runs the task via the project's CLI entry point

---

## Automations

Automations define multi-job workflows. The decorator tracks `.job()` calls via context:

```python
@recompose.automation(
    trigger=recompose.on_push(branches=["main"]) | recompose.on_pull_request(),
)
def ci() -> None:
    """CI pipeline with parallel lint/format and sequential test."""
    lint_job = recompose.job(lint)
    format_job = recompose.job(format_check)

    # Dependency inferred: test depends on lint_job and format_job completing
    test_job = recompose.job(test, needs=[lint_job, format_job])
```

- Name auto-generated from function: `ci` → workflow name "ci"
- No return value needed - jobs tracked via context
- `needs` can be explicit or inferred from input references

### Automation with Inputs

```python
@recompose.automation
def deploy(environment: recompose.InputParam, version: recompose.InputParam = "latest") -> None:
    """Deploy to specified environment."""
    deploy_job = recompose.job(
        deploy_task,
        inputs={
            "env": environment,
            "ver": version,
        },
    )
```

- `recompose.InputParam` in signature → `workflow_dispatch.inputs` in YAML
- Required vs optional determined by presence of default
- Inputs can be passed directly to job inputs

### Job Output References (Inferred Dependencies)

```python
@recompose.automation
def build_and_test() -> None:
    # build_job knows from @task(outputs=["wheel_path"]) that this output exists
    build_job = recompose.job(build_wheel)

    # Dependency AUTOMATICALLY inferred because we reference build_job.get()
    test_job = recompose.job(
        test_wheel,
        inputs={
            "wheel_path": build_job.get("wheel_path"),
        },
    )
```

- `build_job.get("wheel_path")` returns a `JobOutputRef` object
- When a job's inputs contain a `JobOutputRef`, the dependency is inferred
- Validation at construction: error if output name not in task's declared outputs

### Matrix Jobs

```python
@recompose.automation
def test_matrix() -> None:
    test_job = recompose.job(
        test,
        matrix={
            "python": ["3.10", "3.11", "3.12"],
            "os": ["ubuntu-latest", "macos-latest"],
        },
        runs_on="${{ matrix.os }}",
    )
```

### Conditional Jobs

Jobs can have conditions using a lightweight expression algebra (similar to old flow conditionals).
Maps to GHA job-level `if:` spec.

```python
@recompose.automation
def conditional_deploy(
    environment: recompose.InputParam,
    skip_tests: recompose.InputParam = False,
) -> None:
    test_job = recompose.job(
        test,
        # Condition using InputParam - skipped if skip_tests is true
        condition=~skip_tests,
    )

    # Deploy only to prod on main branch
    deploy_job = recompose.job(
        deploy,
        needs=[test_job],
        condition=(environment == "prod") & recompose.github.ref_name.eq("main"),
    )
```

**Expression primitives:**
- `param == value` - InputParam equality
- `param != value` - InputParam inequality
- `~param` - Negation (for boolean params)
- `expr & expr` - AND
- `expr | expr` - OR
- `recompose.github.event_name` - GitHub context references
- `recompose.github.ref_name`
- `recompose.github.ref_type`
- etc.

**Generated GHA:**
```yaml
jobs:
  test:
    if: ${{ inputs.skip_tests != true }}
    ...

  deploy:
    needs: [test]
    if: ${{ inputs.environment == 'prod' && github.ref_name == 'main' }}
    ...
```

**Local execution:**
- Conditions evaluated at runtime with actual parameter values
- Jobs with false conditions are skipped (shown in output)

---

## Generated Workflow Example

For this automation:
```python
@recompose.automation(
    trigger=recompose.on_push(branches=["main"]),
)
def ci() -> None:
    lint_job = recompose.job(lint)
    format_job = recompose.job(format_check)
    test_job = recompose.job(test, needs=[lint_job, format_job])
```

Generates:
```yaml
# GENERATED FILE - DO NOT EDIT
name: ci
on:
  push:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - uses: astral-sh/setup-uv@v4
      - name: lint
        run: ./run lint

  format_check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - uses: astral-sh/setup-uv@v4
      - name: format_check
        run: ./run format_check

  test:
    runs-on: ubuntu-latest
    needs: [lint, format_check]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - uses: astral-sh/setup-uv@v4
      - name: test
        run: ./run test
```

Note: The step command is exactly what a developer would type locally!

---

## App Configuration

The App needs to know the CLI entry point for workflow generation:

```python
app = recompose.App(
    entry_point="./run",  # How to invoke this app in workflows
    working_directory="recompose",
    setup_steps=[
        recompose.checkout(),
        recompose.setup_python("3.12"),
        recompose.setup_uv(),
    ],
    commands=[
        recompose.CommandGroup("Quality", [lint, format_check]),
        recompose.CommandGroup("Testing", [test]),
        recompose.builtin_commands(),
    ],
    automations=[ci, build_and_test],
)
```

- `entry_point`: The command used in workflow steps (e.g., `./run`, `uv run python -m myapp`)
- `setup_steps`: Default GHA setup steps for all jobs (can be overridden per-job)
- `automations`: List of automation functions to generate workflows for

---

## What Gets Removed/Simplified

1. **`@flow` decorator** - Gone. Just use tasks that call tasks.
2. **`FlowPlan`, `TaskNode`, `InputPlaceholder`** - Gone. No graph building.
3. **`@taskclass` and all TaskClass machinery** - Gone entirely. No class-state sync across jobs.
4. **`execute_flow_isolated()`** - Gone. Local execution is just function calls.
5. **`workspace.py` step serialization** - Gone. No inter-step state.
6. **`_run_step.py`** - Gone. Use the app's CLI directly.
7. **Complex type serialization** - Gone. GHA outputs are strings.
8. **`@task` on class methods** - Gone. No way to construct class in GHA job context.

---

## Local vs CI Execution

**Local:**
```bash
./run test
```
- Calls `test()` directly
- If `test` calls other tasks, they execute hierarchically
- Tree-view shows the hierarchy
- Outputs accessible via `result.outputs`

**CI (via automation):**
```bash
./run generate-gha
```
- Generates workflow YAML with separate jobs
- Each job runs ONE task via `./run task_name --args`
- Jobs run in parallel where dependencies allow
- Outputs passed via GHA job outputs mechanism

---

## API Summary

### Decorators
- `@recompose.task(outputs=[...], artifacts=[...], secrets=[...], setup=[...])` - Mark function as task
- `@recompose.automation(trigger=...)` - Mark function as automation

### Task Helpers
- `recompose.set_output(name, value)` - Set a task output (validates against declared outputs)
- `recompose.save_artifact(name, path)` - Save an artifact (validates against declared artifacts)
- `recompose.get_secret(name)` - Get a secret value (validates against declared secrets)
- `recompose.run(...)` - Run subprocess (unchanged)
- `recompose.out(...)`, `recompose.dbg(...)` - Output helpers (unchanged)
- `recompose.step(name)` - Context manager for visual grouping (nested locally, flat `::group::` in GHA)
- `@recompose.step(name)` - Decorator form for helper functions

### Automation Helpers
- `recompose.job(task, inputs={}, needs=[], runs_on=..., matrix={}, condition=...)` - Define a job
- `job.get("output_name")` - Reference a job's output (creates dependency)
- `job.artifact("artifact_name")` - Reference a job's artifact (creates dependency + download)

### Condition Expressions
- `param == value`, `param != value` - Equality/inequality
- `~expr` - Negation
- `expr & expr` - AND
- `expr | expr` - OR
- `recompose.github.event_name`, `.ref_name`, `.ref_type`, etc. - GitHub context

### Dispatchable
- `recompose.make_dispatchable(task, inputs={})` - Create dispatchable workflow for task

### Triggers
- `recompose.on_push(branches=[], tags=[])`
- `recompose.on_pull_request(branches=[])`
- `recompose.on_schedule(cron="...")`
- `recompose.on_workflow_dispatch()`
- Triggers can be combined with `|`

### Input Types
- `recompose.InputParam` - Type hint for automation inputs
- `recompose.Artifact` - Type hint for artifact inputs to tasks
- `recompose.StringInput(default=...)` - String workflow input
- `recompose.BoolInput(default=...)` - Boolean workflow input
- `recompose.ChoiceInput(choices=[...], default=...)` - Choice workflow input

### Setup Steps
- `recompose.checkout()`
- `recompose.setup_python(version)`
- `recompose.setup_uv(version="latest")`
- `recompose.setup_rust(toolchain="stable")`

---

## Resolved Design Decisions

1. **TaskClass**: Removed entirely. No class-state sync across GHA jobs.

2. **Artifacts**: Tasks declare `artifacts=["name"]`, use `save_artifact(name, path)`.
   Automation adds upload/download steps. `recompose.Artifact` type for inputs.

3. **Secrets**: Tasks declare `secrets=["NAME"]`, use `get_secret(name)`.
   GHA gets from `${{ secrets.NAME }}`, local from `~/.recompose/secrets.toml`.

4. **Setup overrides**: Via `@task(setup=[...])` decorator parameter.

5. **Conditional jobs**: Via `condition=` parameter on `job()`.
   Uses expression algebra (`&`, `|`, `~`, `==`, `!=`). Maps to GHA job-level `if:`.
   No step-level conditionals needed since each job runs one task.

## Resolved Design Decisions (continued)

6. **Visual step grouping**: `@step` decorator (or context manager) for output grouping.
   - Takes optional `name` parameter for header
   - **Locally**: Nested tree-view output (full hierarchy)
   - **In GHA**: Flat `::group::` / `::endgroup::` (no nesting support in GHA)
   - Useful for organizing output when tasks have many sub-operations

---

## Implementation Plan

### Phase 1: Core Infrastructure - Task Decorator Enhancements ✅ COMPLETE
- [x] Add `outputs` parameter to `@task` decorator
- [x] Add `artifacts` parameter to `@task` decorator
- [x] Add `secrets` parameter to `@task` decorator
- [x] Add `setup` parameter to `@task` decorator
- [x] Implement `recompose.set_output()` helper (with validation)
- [x] Implement `recompose.save_artifact()` helper (with validation)
- [x] Implement `recompose.get_secret()` helper (with validation)
- [x] Add outputs/artifacts to Result type
- [x] Implement local secrets file (`~/.recompose/secrets.toml`)
- [x] Implement `recompose.step()` context manager and decorator (nested locally, `::group::` in GHA)

### Phase 2: Automation Framework ✅ COMPLETE
- [x] Create `@automation` decorator with context tracking
- [x] Implement `recompose.job()` returning JobSpec
- [x] Implement `JobSpec.get()` returning JobOutputRef (for outputs)
- [x] Implement `JobSpec.artifact()` returning ArtifactRef
- [x] Implement dependency inference from JobOutputRef/ArtifactRef
- [x] Add InputParam type for automation parameters
- [x] Add Artifact type for artifact inputs
- [x] Implement condition expression algebra (reuse/adapt from old expr.py)
- [x] Add `recompose.github.*` context references for conditions

### Phase 3: Triggers ✅ COMPLETE (implemented in Phase 2)
- [x] Implement trigger classes (on_push, on_pull_request, on_schedule, on_workflow_dispatch)
- [x] Implement trigger combination with `|`

### Phase 4: Workflow Generation ✅ COMPLETE
- [x] Update GHA generation for new multi-job model
- [x] Generate jobs using app's entry_point
- [x] Handle job outputs/inputs mapping
- [x] Handle artifact upload/download steps
- [x] Handle secrets in job env
- [x] Handle per-task setup overrides
- [x] Handle matrix jobs
- [x] Handle job conditions (if:)

### Phase 5: Dispatchable
- [ ] Implement `make_dispatchable()` function
- [ ] Generate single-job workflow_dispatch workflows

### Phase 6: Cleanup Old Code
- [ ] Remove `@flow` decorator and FlowPlan/TaskNode/InputPlaceholder
- [ ] Remove `@taskclass` and all TaskClass machinery
- [ ] Remove `workspace.py` step serialization
- [ ] Remove `_run_step.py`
- [ ] Remove `execute_flow_isolated()`

### Phase 7: Migration & Polish
- [ ] Migrate examples to new model
- [ ] Update App class with entry_point and automations
- [ ] Update documentation
- [ ] Ensure all tests pass

---

## Completion Criteria

- [x] `@task(outputs=[...])` works with `set_output()`
- [x] `@task(artifacts=[...])` works with `save_artifact()`
- [x] `@task(secrets=[...])` works with `get_secret()` and local secrets file
- [x] `@task(setup=[...])` overrides default setup steps
- [x] `@automation` creates multi-job workflows via context tracking
- [x] Job dependencies inferred from output/artifact references
- [x] Job conditions work with expression algebra, map to GHA `if:`
- [x] Artifact upload/download steps generated correctly
- [x] Secrets plumbed to job env in GHA
- [ ] `make_dispatchable()` creates single-job workflows
- [x] Generated workflows use app entry_point directly (copy-paste runnable)
- [ ] All examples migrated to new model
- [ ] All old flow/taskclass code removed
- [x] All tests passing (291 tests)
