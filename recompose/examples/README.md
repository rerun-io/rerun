# Recompose Examples

This directory contains both tutorials for learning recompose and real tasks
used in the recompose project's own CI/development workflow.

## Quick Start

```bash
cd recompose

# See all available tasks
uv run python examples/app.py --help

# Run individual tasks
uv run python examples/app.py lint
uv run python examples/app.py test

# Run the CI flow
uv run python examples/app.py ci

# Inspect a flow without running it
uv run python examples/app.py inspect ci
```

## Directory Structure

```
examples/
├── app.py              # Unified entrypoint - run all tasks from here
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
# Run the tutorial
uv run python examples/tutorial/intro_tasks.py --help

# Try individual tasks
uv run python examples/tutorial/intro_tasks.py hello
uv run python examples/tutorial/intro_tasks.py greet --name="Alice"
uv run python examples/tutorial/intro_tasks.py check_tool --tool=git
uv run python examples/tutorial/intro_tasks.py divide --a=10 --b=2
uv run python examples/tutorial/intro_tasks.py divide --a=10 --b=0  # Error case
```

### 2. Task Classes (`tutorial/intro_taskclass.py`)

Learn to group related tasks:
- **`@taskclass` decorator** - Create task groups
- **Shared configuration** - Constructor args become shared CLI options
- **Member tasks** - Methods become sub-commands

```bash
# Run the tutorial
uv run python examples/tutorial/intro_taskclass.py --help

# Try the Counter taskclass
uv run python examples/tutorial/intro_taskclass.py counter.increment --start=10 --by=5
uv run python examples/tutorial/intro_taskclass.py counter.show --start=42

# Try the FileOps taskclass
uv run python examples/tutorial/intro_taskclass.py fileops.list --directory=/tmp
uv run python examples/tutorial/intro_taskclass.py fileops.count --directory=/tmp
```

### 3. Flows (`tutorial/intro_flows.py`)

Learn to compose tasks:
- **`@flow` decorator** - Define task pipelines
- **`.flow()` method** - Wire tasks together
- **Data dependencies** - Pass results between tasks
- **`inspect` command** - View flow structure without running

```bash
# Run the tutorial
uv run python examples/tutorial/intro_flows.py --help

# Run flows
uv run python examples/tutorial/intro_flows.py tool_check
uv run python examples/tutorial/intro_flows.py greeting_pipeline --name="Alice"
uv run python examples/tutorial/intro_flows.py math_pipeline --a=20 --b=4

# Inspect flows without running
uv run python examples/tutorial/intro_flows.py inspect tool_check
uv run python examples/tutorial/intro_flows.py inspect math_pipeline
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
uv run python examples/app.py lint
uv run python examples/app.py format_check
uv run python examples/app.py format  # Modifies files!
```

### Test Tasks (`tasks/test.py`)

| Task | Description | Used In CI? |
|------|-------------|-------------|
| `test` | Run pytest suite | Yes |

```bash
uv run python examples/app.py test
uv run python examples/app.py test --verbose
uv run python examples/app.py test --coverage
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
uv run python examples/app.py ci

# Inspect the CI flow
uv run python examples/app.py inspect ci
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
