# Recompose

A lightweight, typed, pythonic task execution framework.

## Installation

```bash
uv add recompose
```

## Basic Usage

```python
import recompose

@recompose.task
def greet(*, name: str) -> recompose.Result[str]:
    recompose.out(f"Hello, {name}!")
    return recompose.Ok(f"greeted {name}")

# Call directly as a function:
result = greet(name="World")
assert result.ok
print(result.value())  # "greeted World"
```

## Development

See `PLAN.md` for the full vision and `WORK.md` for current progress.
