# NOW

**P11_explicit_registration** - IN PROGRESS

Moving from auto-registration to explicit, organized command registration.

**Target API:**
```python
config = recompose.Config(python_cmd="uv run python", working_directory="recompose")

commands = [
    recompose.CommandGroup("Python", [lint, format, test]),
    recompose.CommandGroup("Rust", [...]),
    recompose.builtin_commands(),
    recompose.CommandGroup("Helpers", [...], hidden=True),
]

recompose.main(config=config, commands=commands, automations=[...])
```

**Why explicit:**
- Better organization for large projects (rerun is huge)
- Control over CLI visibility (some tasks internal-only)
- Command groups for organized help output
- Flat namespace but visual grouping
- Clear what's exposed vs internal

**Implementation phases:**
- P11a: Config class + restructured main()
- P11b: CommandGroup + builtin_commands()
- P11c: Migration + validation

See: `proj/P11_explicit_registration_TODO.md`

# UPCOMING

After P11 completes, next priorities:
1. **Real-world usage in rerun** - Start migrating actual rerun CI tasks
2. **Documentation** - User guide and API reference
3. **Performance optimization** - Profile task execution if needed

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
