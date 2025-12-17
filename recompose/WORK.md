# NOW

**P12_architecture_cleanup** - Codebase organization and cleanup pass

**Status**: Phase 1-3 COMPLETE! Phase 4 (polish) remaining.

**Completed:**
- Phase 1 (Quick Wins): topological sort removal, unused aliases, duplicate git root, GHA docs
- Phase 2 (Naming): renamed `github.py` → `gh_cli.py`, `flowgraph.py` → `plan.py`
- Phase 3 (Code Organization):
  - #3: Consolidated duplicate wrapper code in task.py (extracted shared helpers)
  - #4: Simplified flow.py by removing in-process execution - flows always use subprocess isolation (matches GHA)
  - #5: Moved GitHub handlers from cli.py to gh_cli.py (933→796 lines)
  - Created `tests/flow_test_app.py` as module-level test app for subprocess compatibility

**Remaining (Phase 4 - Polish):**
- #9: Context globals consolidation (nice to have)
- #12: Test coverage improvements (ongoing)

See `proj/P12_architecture_cleanup_TODO.md` for full details.

# UPCOMING

1. **Real-world usage in rerun** - Start migrating actual rerun CI tasks
2. **Documentation** - User guide and API reference

# DEFERRED

**P05c_flows_parallel** - Parallel task execution within flows
- Currently flows execute tasks sequentially (matches GHA step model)
- Defer until clear use case emerges

**Logging integration** - Replace `recompose.out` with Python logging
- Could hook into Python's logging framework for standard patterns
- Defer until more real usage to inform design
- Current `recompose.out` works fine

# RECENTLY COMPLETED

For detailed plans, see `proj/P*_DONE.md` files.

**Recent milestones:**
- **P11_explicit_registration** - DONE. Moved from auto-discovery to explicit registration:
  - Tasks/flows/automations are NOT auto-registered by decorators
  - `main(commands=[...])` builds registry from explicit command list
  - `CommandGroup` for organized CLI help output
  - `Config` dataclass for python_cmd, working_directory
  - `builtin_commands()` returns inspect/generate-gha tasks
  - `_recompose_tasks` dict on @taskclass for explicit registration
  - Registry accessible via context: `get_task_registry()`, `get_flow_registry()`, etc.
- **P10_context_dispatch** - Simplified API: removed `.flow()` method, context-based dispatch
- **P09_workflow_dispatch** - CLI-to-GitHub integration (`--remote`, `--status` flags)
- **Tree-based output** - Visual flow execution with nested subprocess indicators
- **Conditional execution** - `run_if()` context manager with expression algebra
- **P08_ci_integration** - Full GitHub Actions integration, working CI pipeline
- **P07_real_examples** - Real CI/dev workflow for recompose itself

**Earlier work:**
- P01-P06: Foundation (tasks, CLI, subprocess, member tasks, flows, GHA generation)

# ARCHITECTURE DECISIONS

## Library Choices

| Need | Choice | Rationale |
|------|--------|-----------|
| CLI generation | **Click** | Mature, well-documented. We want control over CLI generation. |
| Result/data types | **Pydantic** | Perfect for typed, validatable Result classes. |
| Console output | **Rich** | Great for formatted output, progress bars, etc. |
| Async (later) | **asyncio** | Built-in. Only needed for parallel flow execution. |

## Design Principles

1. **Tasks are just functions** - The `@task` decorator minimally alters the function
2. **CLI is opt-in** - `recompose.main()` builds CLI, but tasks work without it
3. **Result is explicit** - Tasks return `Result[T]` with value + status + output
4. **Context is ambient** - Helpers detect if running inside recompose engine
5. **Explicit registration** - Only commands passed to `main()` are CLI-accessible

## Hierarchy

- **Task** - Single unit of work (Python function with @task)
- **Flow** - Composition of tasks → Single GHA job, workflow_dispatch triggerable
- **Automation** - Orchestrates flows → Uses workflow_run to chain workflows

## Workflow Generation

- Workflows sync to `.github/workflows/` (named `recompose_flow_<name>.yml` / `recompose_automation_<name>.yml`)
- Generated files include header comment identifying them as generated
- CI validates committed workflows match generated output via `generate_gha --check_only`

## Local vs CI Tasks

- **Local-only**: `format` (modifies files), workflow regeneration
- **CI tasks**: `lint`, `format_check`, `test`, `generate_gha --check_only`
