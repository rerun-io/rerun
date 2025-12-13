#!/usr/bin/env python3
"""Example recompose application."""

import recompose


@recompose.task
def greet(*, name: str, count: int = 1) -> recompose.Result[str]:
    """Greet someone multiple times."""
    for _ in range(count):
        recompose.out(f"Hello, {name}!")
    return recompose.Ok("done")


@recompose.task
def add(*, a: int, b: int) -> recompose.Result[int]:
    """Add two numbers together."""
    result = a + b
    recompose.out(f"{a} + {b} = {result}")
    return recompose.Ok(result)


@recompose.task
def failing_task() -> recompose.Result[str]:
    """A task that always fails."""
    raise ValueError("This task intentionally fails!")


if __name__ == "__main__":
    recompose.main()
