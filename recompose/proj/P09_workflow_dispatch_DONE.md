# P09: Workflow Dispatch - CLI-to-GitHub Integration (DONE)

## Goal

Enable ergonomic triggering of GitHub Actions workflows directly from the recompose CLI, with validation to ensure local and remote workflows are in sync.

## Use Cases

1. **Remote execution**: `./run ci --remote` triggers the workflow on GitHub instead of running locally
2. **Status checking**: See the status of workflow runs for a given flow
3. **Sync validation**: Warn/error if local workflow differs from what's on GitHub

## Implementation Plan

### Phase 1: Core Infrastructure

1. **GitHub CLI integration** (`src/recompose/github.py`)
   - Wrapper around `gh` CLI for workflow operations
   - Functions:
     - `list_workflows()` - list all workflows in the repo
     - `get_workflow(name)` - get workflow by name
     - `trigger_workflow(name, inputs)` - dispatch workflow_dispatch event
     - `list_runs(workflow_name)` - list recent runs
     - `get_run_status(run_id)` - get status of a specific run

2. **Workflow mapping**
   - Map flow names to workflow files: `ci` → `recompose_flow_ci.yml`
   - Detect when a flow has a corresponding GHA workflow

### Phase 2: Dispatch Command

1. **Add `--remote` flag to flow execution**
   - When `--remote` is passed, dispatch to GitHub instead of local execution
   - Pass flow parameters as workflow_dispatch inputs

2. **Sync validation before dispatch**
   - Generate workflow YAML locally
   - Fetch workflow YAML from GitHub (via `gh api`)
   - Compare and warn/error if they differ
   - Option to skip validation: `--force`

### Phase 3: Status & Monitoring

1. **Add `status` subcommand**
   - `./run ci --status` - show recent runs of the ci workflow
   - Display: run ID, status, conclusion, started_at, URL

2. **Watch mode** (bonus)
   - `./run ci --remote --watch` - dispatch and wait for completion
   - Poll for status updates
   - Show live output if available

## Technical Details

### GitHub CLI Commands

```bash
# List workflows
gh workflow list

# Trigger workflow
gh workflow run recompose_flow_ci.yml -f param1=value1

# List runs for a workflow
gh run list --workflow=recompose_flow_ci.yml

# Get run details
gh run view <run_id>

# Get workflow file from repo
gh api repos/{owner}/{repo}/contents/.github/workflows/recompose_flow_ci.yml
```

### Workflow-to-Flow Mapping

Convention: `recompose_flow_{flow_name}.yml` maps to flow `{flow_name}`

### CLI Changes

```
# Current
./run ci                    # Run locally
./run ci --inspect          # Inspect flow

# New
./run ci --remote           # Trigger on GitHub
./run ci --remote --watch   # Trigger and wait
./run ci --status           # Show recent runs
./run ci --remote --force   # Skip sync validation
```

## Dependencies

- `gh` CLI must be installed and authenticated
- Repository must have workflow files committed

## Completion Criteria

- [x] `./run ci --remote` triggers workflow on GitHub
- [x] Sync validation warns if workflow is out of date
- [x] `./run ci --status` shows recent run history
- [x] Tests for GitHub CLI wrapper (mocked)
- [ ] Documentation in examples/README.md (deferred - not critical)

## Notes

- Start simple: just dispatch + basic status
- Watch mode is bonus if time permits
- Error messages should be helpful (e.g., "gh not found", "not authenticated")
