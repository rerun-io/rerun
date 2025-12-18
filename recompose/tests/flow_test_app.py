#!/usr/bin/env python3
"""
Test application for flow execution tests.

This module defines flows at module level so they can be invoked via subprocess.
Tests import this module and call the flows.
"""

import recompose
from recompose import Err, Ok, Result

# =============================================================================
# Basic tasks
# =============================================================================


@recompose.task
def step_a() -> Result[str]:
    return Ok("a_result")


@recompose.task
def step_b() -> Result[str]:
    return Ok("b_result")


@recompose.task
def produce(*, value: int) -> Result[int]:
    return Ok(value * 2)


@recompose.task
def consume(*, input_val: int) -> Result[str]:
    return Ok(f"got {input_val}")


@recompose.task
def double(*, value: int) -> Result[int]:
    return Ok(value * 2)


# =============================================================================
# Failure tasks
# =============================================================================


@recompose.task
def ok_task() -> Result[str]:
    return Ok("fine")


@recompose.task
def failing_task() -> Result[str]:
    return Err("failed!")


@recompose.task
def never_run() -> Result[str]:
    return Ok("should not see this")


@recompose.task
def throwing_task() -> Result[str]:
    raise ValueError("Task exception")


# =============================================================================
# Math tasks
# =============================================================================


@recompose.task
def multiply(*, x: int, y: int) -> Result[int]:
    return Ok(x * y)


@recompose.task
def add(*, x: int, y: int) -> Result[int]:
    return Ok(x + y)


# =============================================================================
# Greeting/echo tasks (for parameterized flow tests)
# =============================================================================


@recompose.task
def greet(*, name: str) -> Result[str]:
    """A task that greets someone."""
    return Ok(f"Hello, {name}!")


@recompose.task
def count_task(*, n: int = 10) -> Result[int]:
    """A task that counts."""
    return Ok(n)


@recompose.task
def echo(*, message: str) -> Result[str]:
    """A task that echoes a message."""
    return Ok(message)


# =============================================================================
# Flows
# =============================================================================


@recompose.flow
def simple_flow() -> None:
    """A simple two-step flow."""
    step_a()
    step_b()


@recompose.flow
def dependent_flow() -> None:
    """Flow with task dependencies."""
    produced = produce(value=5)
    consume(input_val=produced.value())


@recompose.flow
def arg_flow(*, initial: int) -> None:
    """Flow with external arguments."""
    double(value=initial)


@recompose.flow
def fail_fast_flow() -> None:
    """Flow that should fail on the second task."""
    ok_task()
    failing_task()
    never_run()


@recompose.flow
def failure_flow() -> None:
    """Flow with a failing task that has a dependent."""
    r = failing_task()
    echo(message=r.value())  # Won't run - dep failed


@recompose.flow
def throwing_flow() -> None:
    """Flow with a task that raises an exception."""
    throwing_task()


@recompose.flow
def math_flow(*, a: int, b: int) -> None:
    """Flow that chains math operations."""
    mul_result = multiply(x=a, y=b)
    add(x=mul_result.value(), y=10)


@recompose.flow
def parameterized_flow(*, name: str, count: int = 1) -> None:
    """Flow with multiple parameters."""
    # Just use the params in a task
    produce(value=count)


# =============================================================================
# Parameterized flow tests
# =============================================================================


@recompose.flow
def flow_with_required_param(*, name: str) -> None:
    """A flow that requires a name parameter."""
    greet(name=name)


@recompose.flow
def flow_with_mixed_params(*, name: str, count_to: int = 10) -> None:
    """A flow with both required and optional parameters."""
    greet(name=name)
    count_task(n=count_to)


@recompose.flow
def flow_with_param_reuse(*, message: str) -> None:
    """A flow that uses the same param in multiple tasks."""
    echo(message=message)
    echo(message=message)


@recompose.flow
def flow_with_value_composition() -> None:
    """Flow that demonstrates .value() composition."""
    result = greet(name="World")
    echo(message=result.value())


@recompose.flow
def flow_with_optional_only() -> None:
    """Flow that uses task with optional param."""
    count_task()


# =============================================================================
# App Instance (must be at module level for subprocess isolation)
# =============================================================================

app = recompose.App(
    commands=[
        recompose.CommandGroup(
            "Flows",
            [
                simple_flow,
                dependent_flow,
                arg_flow,
                fail_fast_flow,
                failure_flow,
                throwing_flow,
                math_flow,
                parameterized_flow,
                flow_with_required_param,
                flow_with_mixed_params,
                flow_with_param_reuse,
                flow_with_value_composition,
                flow_with_optional_only,
            ],
        ),
    ],
)

# =============================================================================
# CLI Entry Point
# =============================================================================

if __name__ == "__main__":
    app.main()
