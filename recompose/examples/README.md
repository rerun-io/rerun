# Recompose Examples

Tasks and automations for the recompose project's own CI/development workflow.

## Quick Start

```bash
cd recompose

# See all available commands
./run --help

# Run individual tasks
./run lint
./run test

# Run the CI automation
./run ci

# Inspect an automation
./run inspect --target=ci
```

## Directory Structure

```
examples/
├── app.py              # Main app (use via ./run)
├── tasks/              # Individual tasks
│   ├── lint.py         # lint, lint_all, format_check, format_code
│   ├── test.py         # test
│   └── build.py        # build_wheel, create_test_venv, etc.
└── automations/        # Multi-job workflows
    └── ci.py           # CI pipeline automation
```

## Tasks

| Task | Description |
|------|-------------|
| `lint` | Run ruff linter |
| `lint_all` | Lint + mypy + format check + GHA sync |
| `format_check` | Check code formatting |
| `format_code` | Apply formatting fixes |
| `test` | Run pytest suite |
| `build_wheel` | Build distribution wheel |

## Automations

| Automation | Description |
|------------|-------------|
| `ci` | CI pipeline: lint_all and test in parallel |

## Core Concepts

| Concept | Decorator | Purpose |
|---------|-----------|---------|
| Task | `@recompose.task` | Single unit of work |
| Automation | `@recompose.automation` | Multi-job workflow |

| Helper | Purpose |
|--------|---------|
| `recompose.Ok(value)` | Create success result |
| `recompose.Err(message)` | Create failure result |
| `recompose.out(text)` | Print to console |
| `recompose.run(*args)` | Execute subprocess |
| `recompose.job(task)` | Add task as job in automation |
