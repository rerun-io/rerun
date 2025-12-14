# P06: GitHub Actions Workflow Generation

**Status:** IN_PROGRESS

## Goal

Enable recompose flows and automations to generate GitHub Actions workflow YAML files.
Users generate workflows, review them, and commit them to source control.

## Architecture

### Hierarchy

```
Task       → Single unit of work (a Python function)
Flow       → Composition of tasks → Single GHA job, workflow_dispatch triggerable
Automation → Orchestrates flows   → Uses workflow_run to chain workflow executions
```

### Flows → workflow_dispatch

Each flow generates a workflow file with `workflow_dispatch` trigger:

```yaml
# .github/workflows/build_pipeline.yml
name: build_pipeline
on:
  workflow_dispatch:
    inputs:
      repo:
        description: 'Repository branch'
        required: false
        default: 'main'

jobs:
  build_pipeline:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup
        run: python script.py build_pipeline --setup --workspace .recompose --repo ${{ inputs.repo }}
      - name: 1_fetch_source
        run: python script.py build_pipeline --step 1_fetch_source --workspace .recompose
      - name: 2_compile_source
        run: python script.py build_pipeline --step 2_compile_source --workspace .recompose
      # ...
```

**Key points:**
- Flow parameters become `workflow_dispatch.inputs`
- Each flow is independently triggerable from GHA UI
- Failed flows can be manually re-run without re-running everything

### Automations → workflow_run

Automations orchestrate multiple flows via `workflow_run`:

```python
@recompose.automation(
    gha_on={"schedule": [{"cron": "0 0 * * *"}]}  # Nightly
)
def nightly_release():
    # Dispatch flows (potentially to different runners)
    linux = build_pipeline.dispatch(repo="main")
    mac = build_pipeline.dispatch(repo="main", gha_runs_on="macos-latest")

    # This flow runs after both complete
    publish_release.dispatch(version="nightly")
```

Generates:
```yaml
# .github/workflows/nightly_release.yml
name: nightly_release
on:
  schedule:
    - cron: '0 0 * * *'
  workflow_run:
    workflows: [build_pipeline]
    types: [completed]

jobs:
  orchestrate:
    runs-on: ubuntu-latest
    steps:
      - name: Dispatch build_pipeline (linux)
        uses: benc-uk/workflow-dispatch@v1
        with:
          workflow: build_pipeline.yml
          inputs: '{"repo": "main"}'
      # ... coordination logic
```

**Key points:**
- Automations compose flows, not tasks
- Uses `workflow_run` for GHA UI to show workflow chains
- Each flow runs as a separate workflow (can be on different runners)

## Implementation Phases

### Phase 1: Flow → YAML Generation (MVP)

**Goal:** Generate valid workflow YAML from a flow.

1. **Add `gha.py` module:**
   ```python
   @dataclass
   class WorkflowSpec:
       name: str
       on: dict[str, Any]  # Triggers
       jobs: dict[str, JobSpec]

   @dataclass
   class JobSpec:
       runs_on: str
       steps: list[StepSpec]

   def render_flow_workflow(flow_info: FlowInfo) -> str:
       """Generate workflow YAML for a flow."""
   ```

2. **Add CLI command:**
   ```bash
   ./app.py generate-gha build_pipeline
   # Outputs YAML to stdout

   ./app.py generate-gha build_pipeline --output .github/workflows/build_pipeline.yml
   # Writes to file
   ```

3. **Flow parameters → workflow_dispatch inputs:**
   - Introspect flow signature
   - Map types: `str`, `int`, `bool` → GHA input types
   - Include defaults

4. **Step generation:**
   - Setup step: `--setup --workspace .recompose`
   - Task steps: `--step {name} --workspace .recompose`

5. **Local validation with actionlint:**
   - Integrate [actionlint](https://github.com/rhysd/actionlint) for workflow validation
   - `generate-gha --validate` runs actionlint on generated output
   - Clear error messages if validation fails
   - Can install via: `brew install actionlint` or `go install github.com/rhysd/actionlint/cmd/actionlint@latest`

**Deliverables:**
- `generate-gha` CLI command works
- `--validate` flag runs actionlint on output
- Generated workflow is valid YAML that passes actionlint
- Can manually run generated workflow in GHA

### Phase 2: GHA Setup Actions

**Goal:** Support common GHA setup actions as virtual tasks in flows.

1. **Create `recompose.gha` namespace:**
   ```python
   # Virtual tasks that render as `uses:` steps
   recompose.gha.checkout()                    # → actions/checkout@v4
   recompose.gha.setup_python(version="3.11") # → actions/setup-python@v5
   recompose.gha.setup_uv()                   # → astral-sh/setup-uv@v4
   recompose.gha.setup_rust(toolchain="stable")
   recompose.gha.cache(path="~/.cache", key="...")
   ```

2. **Virtual task behavior:**
   - Local execution: No-op, return `Ok(None)`
   - `plan()`: Include in graph as special nodes with `is_gha_action=True`
   - YAML render: Emit `uses:` instead of `run:`

3. **Ordering:** GHA actions appear before task steps

**Example flow:**
```python
@recompose.flow
def build_pipeline(*, repo: str = "main") -> None:
    recompose.gha.checkout()
    recompose.gha.setup_python(version="3.11")
    recompose.gha.setup_uv()

    source = fetch_source.flow(repo=repo)
    binary = compile_source.flow(source_dir=source)
```

**Deliverables:**
- Virtual GHA tasks work in flows
- Running locally skips them gracefully
- Generated YAML includes proper `uses:` actions

### Phase 3: Automations

**Goal:** Implement `@automation` decorator and workflow_run orchestration.

1. **`@automation` decorator with GHA config:**
   ```python
   @recompose.automation(
       gha_on={"schedule": [{"cron": "0 0 * * *"}]},  # When to trigger
       gha_runs_on="ubuntu-latest",                   # Orchestration runner
       gha_env={"RUST_LOG": "debug"},                 # Environment
       gha_timeout_minutes=30,                        # Timeout
   )
   def nightly_release():
       build_pipeline.dispatch(repo="main")
       publish_release.dispatch()
   ```

2. **`.dispatch()` method on flows:**
   - Returns a handle representing the dispatched workflow
   - Can pass flow parameters

3. **YAML generation for automations:**
   - Include configured triggers (schedule, push, workflow_run, etc.)
   - Steps to dispatch child workflows

**Deliverables:**
- `@automation` decorator works with GHA config
- Automations generate valid workflow YAML
- workflow_run chaining works in GHA

### Phase 4: Secrets and Environment

**Goal:** Handle secrets and environment variables.

1. **Secret references:**
   ```python
   token = recompose.gha.secret("GITHUB_TOKEN")
   # In YAML: ${{ secrets.GITHUB_TOKEN }}
   ```

2. **Local secrets:**
   - Read from `~/.recompose/secrets.toml`
   - Clear error if required secret missing

**Deliverables:**
- Secrets work in both local and CI
- Environment variables configurable

## File Structure

```
src/recompose/
├── gha.py              # NEW: GHA generation, virtual tasks, WorkflowSpec
├── automation.py       # NEW: @automation decorator
├── flow.py             # Modified: GHA config in decorator
├── cli.py              # Modified: generate-gha command
└── ...
```

## Test Strategy

1. **Unit tests:**
   - YAML generation produces valid output
   - Flow params → workflow_dispatch inputs
   - Virtual tasks render correctly

2. **Integration tests:**
   - Generate workflow from example flow
   - Validate with actionlint (if installed)
   - Compare against expected output snapshots

3. **Manual validation:**
   - Run generated workflow in test repo
   - Verify matches `run_isolated()` behavior

## Design Decisions

1. **Workflows are manually committed** - User generates, reviews, commits. Not automatic.

2. **Flows = workflow_dispatch only** - Each flow is independently triggerable with inputs. No GHA config on flows - they're always workflow_dispatch.

3. **Automations have GHA config** - Triggers (schedule, push, PR), runner, env, timeout all configured on automations.

4. **Automations = workflow_run** - GHA UI shows workflow chains nicely.

5. **Clean separation** - Automations dispatch flows, they don't inline them. If you want one job, use a flow with more tasks.

## Success Criteria

**Phase 1 complete when:**
- [ ] `generate-gha` CLI command works
- [ ] `--validate` flag runs actionlint
- [ ] Generated workflow passes actionlint
- [ ] Flow parameters become workflow_dispatch inputs

**P06 complete when:**
- [ ] Phases 1-3 implemented
- [ ] Example workflows for `build_pipeline` and a simple automation
- [ ] Documentation for GHA generation
