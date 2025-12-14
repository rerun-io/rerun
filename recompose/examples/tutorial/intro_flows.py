#!/usr/bin/env python3
"""
Tutorial: Flows

This tutorial introduces flows for composing tasks:
- The @flow decorator creates task pipelines
- Tasks are wired together using the .flow() method
- Results from one task can be passed to dependent tasks
- Flows can be inspected before execution

Run this file to see all available commands:
    uv run python examples/tutorial/intro_flows.py --help

Run flows:
    uv run python examples/tutorial/intro_flows.py tool_check
    uv run python examples/tutorial/intro_flows.py greeting_pipeline --name="Alice"
    uv run python examples/tutorial/intro_flows.py math_pipeline --a=20 --b=4

Inspect flows without running:
    uv run python examples/tutorial/intro_flows.py inspect tool_check
    uv run python examples/tutorial/intro_flows.py inspect greeting_pipeline
"""

# Import tasks from intro_tasks to compose into flows
from intro_tasks import check_tool, divide, greet

import recompose

# =============================================================================
# ADDITIONAL TASKS FOR FLOWS
# =============================================================================
#
# These tasks are designed to be composed in flows.
# Notice the dependency parameters - they receive results from upstream tasks.


@recompose.task
def format_result(*, message: str, tool_version: str) -> recompose.Result[str]:
    """
    Format a greeting with tool info.

    This task depends on results from greet and check_tool.

    Args:
        message: Result from greet task
        tool_version: Result from check_tool task
    """
    formatted = f"{message} (using {tool_version})"
    recompose.out(formatted)
    return recompose.Ok(formatted)


@recompose.task
def multiply(*, value: float, factor: int = 2) -> recompose.Result[float]:
    """
    Multiply a value by a factor.

    Args:
        value: Input value (can come from another task)
        factor: Multiplication factor
    """
    result = value * factor
    recompose.out(f"{value} * {factor} = {result}")
    return recompose.Ok(result)


@recompose.task
def summarize(*, result: float) -> recompose.Result[str]:
    """
    Summarize a calculation result.

    Args:
        result: Final calculated value
    """
    summary = f"Final result: {result}"
    recompose.out(summary)
    return recompose.Ok(summary)


# =============================================================================
# BASIC FLOW
# =============================================================================
#
# Flows use @recompose.flow decorator and wire tasks together with .flow()


@recompose.flow
def tool_check() -> None:
    """
    Check for common development tools.

    This flow runs check_tool for multiple tools in sequence.

    The .flow() method:
    - Registers the task in the flow graph
    - Returns a placeholder that can be passed to dependent tasks
    - Executes tasks in dependency order when the flow runs
    """
    check_tool.flow(tool="git")
    check_tool.flow(tool="python")
    check_tool.flow(tool="uv")


# =============================================================================
# FLOW WITH PARAMETERS
# =============================================================================
#
# Flows can take parameters that are passed to tasks.


@recompose.flow
def greeting_pipeline(*, name: str = "World") -> None:
    """
    A pipeline that greets and checks tools.

    Flow parameters become CLI options:
        greeting_pipeline --name="Alice"

    Tasks are wired together by passing .flow() results:
        greeting = greet.flow(name=name)  # Returns placeholder
        format_result.flow(message=greeting)  # Uses placeholder
    """
    # These tasks run in parallel (no dependencies between them)
    greeting = greet.flow(name=name)
    tool_version = check_tool.flow(tool="python")

    # This task depends on both above tasks completing
    format_result.flow(message=greeting, tool_version=tool_version)


# =============================================================================
# FLOW WITH DATA DEPENDENCIES
# =============================================================================
#
# Results flow through the pipeline - each task receives upstream results.


@recompose.flow
def math_pipeline(*, a: int = 10, b: int = 2) -> None:
    """
    A math pipeline demonstrating data flow.

    Shows how results from one task become inputs to the next:
    1. divide(a, b) -> quotient
    2. multiply(quotient, factor) -> product
    3. summarize(product) -> summary

    Try: math_pipeline --a=20 --b=4
    """
    # Step 1: Divide
    quotient = divide.flow(a=a, b=b)

    # Step 2: Multiply the result
    product = multiply.flow(value=quotient, factor=3)

    # Step 3: Summarize
    summarize.flow(result=product)


# =============================================================================
# FLOW WITH ERROR HANDLING
# =============================================================================
#
# When a task fails, the flow stops and reports the error.


@recompose.flow
def risky_pipeline(*, a: int = 10, b: int = 0) -> None:
    """
    A pipeline that might fail.

    If divide fails (b=0), the flow stops and multiply/summarize don't run.

    Try: risky_pipeline --a=10 --b=2  (succeeds)
    Try: risky_pipeline --a=10 --b=0  (fails at divide)
    """
    quotient = divide.flow(a=a, b=b)
    product = multiply.flow(value=quotient, factor=5)
    summarize.flow(result=product)


# =============================================================================
# ENTRYPOINT
# =============================================================================

if __name__ == "__main__":
    recompose.main()
