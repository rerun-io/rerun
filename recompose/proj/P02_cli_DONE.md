# P02: CLI Generation

**Status:** DONE
**Goal:** Auto-generate CLI subcommands from task signatures.

## Scope

By the end, we should be able to:

```python
#!/usr/bin/env python3
import recompose

@recompose.task
def greet(*, name: str, count: int = 1) -> recompose.Result[str]:
    """Greet someone multiple times."""
    for _ in range(count):
        recompose.out(f"Hello, {name}!")
    return recompose.Ok("done")

@recompose.task
def add(*, a: int, b: int) -> recompose.Result[int]:
    """Add two numbers."""
    return recompose.Ok(a + b)

recompose.main()
```

Then run:
```bash
> ./app.py greet --name=World --count=2

▶ greet
Hello, World!
Hello, World!
✓ greet succeeded in 0.01s
→ done

> ./app.py add --a=2 --b=3

▶ add
✓ add succeeded in 0.00s
→ 5

> ./app.py --help
Usage: app.py [OPTIONS] COMMAND [ARGS]...

Commands:
  greet  Greet someone multiple times.
  add    Add two numbers.
```

## Tasks

- [x] Create src/recompose/cli.py with main() and CLI builder
- [x] Introspect task signatures and generate Click commands
- [x] Support types: str, int, float, bool, Path, Optional, Enum
- [x] Handle keyword-only arguments (required vs optional)
- [x] Format output with Rich (task header, result, timing)
- [x] Add --debug flag for verbose output
- [x] Write tests (10 CLI tests)
- [x] Update __init__.py exports

## Implementation Notes

### CLI Structure

Using Click to build the CLI:

```python
import click
from rich.console import Console

console = Console()

def main(name: str | None = None) -> None:
    """Build and run the CLI from registered tasks."""

    @click.group(name=name or "recompose")
    @click.option("--debug/--no-debug", default=False, help="Enable debug output")
    def cli(debug: bool) -> None:
        set_debug(debug)

    # Add a command for each registered task
    for task_name, task_info in get_registry().items():
        cmd = build_command(task_info)
        cli.add_command(cmd)

    cli()
```

### Type Mapping

| Python Type | Click Type | Notes |
|-------------|------------|-------|
| str | STRING | |
| int | INT | |
| float | FLOAT | |
| bool | BOOL | Use --flag/--no-flag |
| Path | PATH | |
| Optional[T] | T | Not required |
| Enum | Choice | Use enum values |

### Output Format

```
▶ task_name

[task output here]

✓ task_name succeeded in 0.05s
→ return_value

# OR on failure:

✗ task_name failed in 0.05s
Error: ValueError: something went wrong
```

## Definition of Done

- [x] `./app.py --help` shows all registered tasks
- [x] `./app.py task_name --help` shows task arguments
- [x] `./app.py task_name --arg=value` runs the task
- [x] Output is formatted nicely with Rich
- [x] --debug flag enables debug output
- [x] Tests pass (34 total tests)
