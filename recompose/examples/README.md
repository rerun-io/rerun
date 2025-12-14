# Recompose Examples

This directory contains both tutorials for learning recompose and real tasks
used in the recompose project's own CI/development workflow.

## Quick Start

```bash
cd recompose

# See all available tasks
./run --help

# Run individual tasks
./run lint
./run test

# Run the CI flow
./run ci

# Inspect a flow without running it
./run inspect ci
```

## Directory Structure

```
recompose/
├── run                     # Entry point script
└── examples/
    ├── __init__.py         # Package marker
    ├── app.py              # Main app (use via ./run)
    ├── README.md           # This file
    │
    ├── tutorial/           # Learning recompose (start here!)
    │   ├── intro_tasks.py      # 1. Basic tasks, Results, subprocess
    │   ├── intro_taskclass.py  # 2. Task classes for grouped operations
    │   └── intro_flows.py      # 3. Composing tasks into flows
    │
    ├── tasks/              # Real tasks for recompose project
    │   ├── __init__.py
    │   ├── lint.py             # lint, format_check, format
    │   └── test.py             # test
    │
    └── flows/              # Real flows for CI
        ├── __init__.py
        └── ci.py               # CI pipeline flow
```

## Tutorial: Learning Recompose

Work through the tutorials in order. Each builds on the previous one.

### 1. Tasks (`tutorial/intro_tasks.py`)

Learn the fundamentals:
- **`@task` decorator** - Turn functions into tasks
- **`Result[T]`** - Return `Ok(value)` or `Err(message)`
- **CLI generation** - Function parameters become CLI options
- **`recompose.out()`** - Output to console
- **`recompose.run()`** - Execute subprocesses

```bash
# Run the tutorial (tutorials are standalone scripts)
uv run python -m examples.tutorial.intro_tasks --help

# Try individual tasks
uv run python -m examples.tutorial.intro_tasks hello
uv run python -m examples.tutorial.intro_tasks greet --name="Alice"
uv run python -m examples.tutorial.intro_tasks check_tool --tool=git
uv run python -m examples.tutorial.intro_tasks divide --a=10 --b=2
uv run python -m examples.tutorial.intro_tasks divide --a=10 --b=0  # Error case
```

### 2. Task Classes (`tutorial/intro_taskclass.py`)

Learn to group related tasks:
- **`@taskclass` decorator** - Create task groups
- **Shared configuration** - Constructor args become shared CLI options
- **Member tasks** - Methods become sub-commands

```bash
# Run the tutorial
uv run python -m examples.tutorial.intro_taskclass --help

# Try the Counter taskclass
uv run python -m examples.tutorial.intro_taskclass counter.increment --start=10 --by=5
uv run python -m examples.tutorial.intro_taskclass counter.show --start=42

# Try the FileOps taskclass
uv run python -m examples.tutorial.intro_taskclass fileops.list --directory=/tmp
uv run python -m examples.tutorial.intro_taskclass fileops.count --directory=/tmp
```

### 3. Flows (`tutorial/intro_flows.py`)

Learn to compose tasks:
- **`@flow` decorator** - Define task pipelines
- **`.flow()` method** - Wire tasks together
- **Data dependencies** - Pass results between tasks
- **`inspect` command** - View flow structure without running

```bash
# Run the tutorial
uv run python -m examples.tutorial.intro_flows --help

# Run flows
uv run python -m examples.tutorial.intro_flows tool_check
uv run python -m examples.tutorial.intro_flows greeting_pipeline --name="Alice"
uv run python -m examples.tutorial.intro_flows math_pipeline --a=20 --b=4

# Inspect flows without running
uv run python -m examples.tutorial.intro_flows inspect tool_check
uv run python -m examples.tutorial.intro_flows inspect math_pipeline
```

## Real Tasks

The `tasks/` directory contains the actual development workflow tasks.

### Lint Tasks (`tasks/lint.py`)

| Task | Description | Used In CI? |
|------|-------------|-------------|
| `lint` | Run ruff linter | Yes |
| `format_check` | Check formatting | Yes |
| `format` | Apply formatting fixes | No (local only) |

```bash
./run lint
./run format_check
./run format  # Modifies files!
```

### Test Tasks (`tasks/test.py`)

| Task | Description | Used In CI? |
|------|-------------|-------------|
| `test` | Run pytest suite | Yes |

```bash
./run test
./run test --verbose
./run test --coverage
```

## Real Flows

The `flows/` directory contains CI pipeline definitions.

### CI Flow (`flows/ci.py`)

The `ci` flow runs the full CI pipeline:
1. `lint` - Check for code quality issues
2. `format_check` - Verify code formatting
3. `test` - Run the test suite

```bash
# Run the full CI pipeline
./run ci

# Inspect the CI flow
./run inspect ci
```

## Core Concepts Summary

| Concept | Decorator | Purpose |
|---------|-----------|---------|
| Task | `@recompose.task` | Single unit of work |
| Task Class | `@recompose.taskclass` | Group of related tasks |
| Flow | `@recompose.flow` | Pipeline of tasks |

| Helper | Purpose |
|--------|---------|
| `recompose.Ok(value)` | Create success result |
| `recompose.Err(message)` | Create failure result |
| `recompose.out(text)` | Print to console |
| `recompose.dbg(text)` | Debug output (with --debug) |
| `recompose.run(*args)` | Execute subprocess |
