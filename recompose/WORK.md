# NOW

Working on: **P04_member_tasks** - Class-based tasks with member methods.

See: `proj/P04_member_tasks_TODO.md` (to be created)

# COMPLETED

- **P03_subprocess** - Subprocess helpers: `recompose.run()`, `RunResult`, streaming/capture modes. See `proj/P03_subprocess_DONE.md`
- **P02_cli** - CLI generation with Click. `recompose.main()` exposes tasks as subcommands. See `proj/P02_cli_DONE.md`
- **P01_foundation** - Core package with `@task`, `Result`, `Ok`/`Err`, `out`/`dbg`. See `proj/P01_foundation_DONE.md`

# UPCOMING

1. **P05_flows** - Task composition and dependency graphs
2. **P06_gha_generation** - GitHub Actions workflow generation from flows

# ARCHITECTURE DECISIONS

## Library Choices

After researching options, here are the recommended dependencies:

| Need | Choice | Rationale |
|------|--------|-----------|
| CLI generation | **Click** | Mature, well-documented. Typer is nice but adds indirection. We want control over CLI generation. |
| Result/data types | **Pydantic** | Perfect for typed, validatable Result classes. Can be subclassed cleanly. |
| Console output | **Rich** | Already in rerun deps. Great for formatted output, progress bars, etc. |
| Async (later) | **asyncio** | Built-in. Only needed when we get to parallel flow execution. |

Key insight: We should NOT depend heavily on existing task frameworks (doit, invoke) because:
- They have their own opinions about task discovery and execution
- Our flow→GitHub Actions rendering is unique
- We want tasks to work as normal Python functions when not using CLI

## Design Principles

1. **Tasks are just functions** - The `@task` decorator should minimally alter the function. It should still be callable normally.
2. **CLI is opt-in** - `recompose.main()` builds CLI from registered tasks, but tasks can be imported/used without it.
3. **Result is explicit** - Tasks return `Result[T]` which wraps the value + status + captured output.
4. **Context is ambient** - Helpers like `recompose.out()` detect if running inside recompose engine and behave accordingly.

## Package Structure

```
recompose/
├── pyproject.toml       # Package config, uv managed
├── src/
│   └── recompose/
│       ├── __init__.py  # Public API exports
│       ├── task.py      # @task decorator, registry
│       ├── result.py    # Result type
│       ├── context.py   # Execution context, out/dbg helpers
│       ├── cli.py       # CLI generation
│       ├── subprocess.py # Subprocess helpers
│       └── ...
├── tests/
│   └── ...
└── proj/                # Sub-project planning docs
    └── ...
```

# SUB-PROJECT OVERVIEW

## P01: Foundation (MVP Core)
**Goal:** Working package with `@task` decorator and basic `Result` type.

- Package scaffolding with pyproject.toml
- Basic `@task` decorator that registers tasks
- `Result[T]` type with success/failure status
- `recompose.out()` / `recompose.dbg()` helpers
- Simple `recompose.main()` that lists tasks

**Completion criteria:** Can define a task, call it as a function, and it returns a Result.

## P02: CLI Generation
**Goal:** Auto-generate CLI subcommands from task signatures.

- Introspect task function signatures
- Map Python types to CLI arguments (str, int, float, bool, Path, Enum)
- Handle defaults, keyword-only args
- Generate help text from docstrings
- Pretty output with Rich

**Completion criteria:** `./app.py my_task --arg1=foo` works and shows formatted output.

## P03: Subprocess Helpers
**Goal:** Easy way to run external commands with good output handling.

- `recompose.run("cargo", "build", ...)` helper
- Stream or capture stdout/stderr
- Return subprocess result with exit code
- Integration with Result type

**Completion criteria:** Can write tasks that shell out to cargo/uv/etc cleanly.

## P04: Member Tasks
**Goal:** Support `@task` on class methods.

- `__init__` becomes a factory task
- Methods become tasks with implicit `self`
- CLI combines class args + method args

**Completion criteria:** The `Venv` example from PLAN.md works.

## P05: Flows
**Goal:** Compose tasks into dependency graphs.

- `@flow` decorator
- Tasks within flows build a DAG at "compile" time
- Execute via subprocess (each task is a separate process)
- Pass data between tasks via serialized results

**Completion criteria:** Can define multi-step flows that execute correctly.

## P06: GitHub Actions Generation
**Goal:** Render flows as GHA workflow YAML.

- Map flow steps to GHA job steps
- Handle GHA-specific placeholder tasks (setup-rust, etc.)
- Output valid workflow YAML

**Completion criteria:** Can generate a working GHA workflow from a flow definition.

# NOTES

- Keep it simple. Don't over-engineer early.
- Write tests as we go.
- Commit frequently with clear messages.
- Each sub-project should be usable independently before moving on.
