# Recompose Examples

This directory contains recompose examples and the actual dev workflow tasks for the recompose project itself.

## Structure

```
examples/
├── README.md           # This file
├── dev_tasks.py        # Real dev workflow tasks (lint, format, test)
└── demos/              # Demo/tutorial examples
    ├── flow_demo.py         # Demonstrates flows and task composition
    ├── member_tasks_demo.py # Demonstrates class-based tasks (@taskclass)
    └── subprocess_demo.py   # Demonstrates subprocess helpers
```

## Dev Tasks

The `dev_tasks.py` file contains the actual development workflow tasks for recompose:

```bash
# Run linting
uv run python examples/dev_tasks.py lint

# Check formatting (doesn't modify files)
uv run python examples/dev_tasks.py format-check

# Apply formatting fixes
uv run python examples/dev_tasks.py format

# Run tests
uv run python examples/dev_tasks.py test

# See all available tasks
uv run python examples/dev_tasks.py --help
```

## Demo Examples

The `demos/` directory contains tutorial examples that demonstrate recompose features:

- **flow_demo.py** - Shows how to compose tasks into flows with dependencies
- **member_tasks_demo.py** - Shows class-based tasks using `@taskclass`
- **subprocess_demo.py** - Shows subprocess execution with `recompose.run()`

Run demos with:
```bash
uv run python examples/demos/flow_demo.py --help
uv run python examples/demos/member_tasks_demo.py --help
uv run python examples/demos/subprocess_demo.py --help
```
