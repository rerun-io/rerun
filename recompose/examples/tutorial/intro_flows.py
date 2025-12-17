#!/usr/bin/env python3
"""
Tutorial: Flows

This tutorial introduces flows for composing tasks:
- The @flow decorator creates task pipelines
- Tasks automatically detect they're in a flow and build the graph
- Use .value() to pass results from one task to another
- Use run_if() for conditional task execution
- Flows can be inspected before execution

Type-safe pattern:
    result = task_a(arg="value")      # Returns Result[T] to type checker
    task_b(input=result.value())      # .value() gives T to type checker

At runtime inside a @flow, task calls return TaskNodes that track dependencies.
The .value() method returns the TaskNode itself, enabling proper wiring.

Run this file to see all available commands:
    uv run python -m examples.tutorial.intro_flows --help

Run flows:
    uv run python -m examples.tutorial.intro_flows tool_check
    uv run python -m examples.tutorial.intro_flows greeting_pipeline --name="Alice"
    uv run python -m examples.tutorial.intro_flows math_pipeline --a=20 --b=4
    uv run python -m examples.tutorial.intro_flows conditional_pipeline
    uv run python -m examples.tutorial.intro_flows conditional_pipeline --run_extra

Inspect flows without running:
    uv run python -m examples.tutorial.intro_flows inspect --target=tool_check
    uv run python -m examples.tutorial.intro_flows inspect --target=conditional_pipeline
"""

import recompose

# Import tasks from intro_tasks to compose into flows
from .intro_tasks import check_tool, divide, greet

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
    Multiply a value by a factor

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
# Flows use @recompose.flow decorator and wire tasks together by calling them


@recompose.flow
def tool_check() -> None:
    """
    Check for common development tools.

    This flow runs check_tool for multiple tools in sequence.

    Tasks called inside a @flow:
    - Automatically register in the flow graph
    - Return placeholders that can be passed to dependent tasks
    - Execute in dependency order when the flow runs
    """
    check_tool(tool="git")
    check_tool(tool="python")
    check_tool(tool="uv")


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

    Tasks are wired together using .value() to pass results:
        greeting = greet(name=name)           # Returns Result[str]
        format_result(message=greeting.value()) # .value() gives str
    """
    # These tasks run in parallel (no dependencies between them)
    greeting = greet(name=name)
    tool_version = check_tool(tool="python")

    # This task depends on both above tasks completing
    # Use .value() to extract the result for type-safe passing
    format_result(message=greeting.value(), tool_version=tool_version.value())


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
    2. multiply(quotient.value(), factor) -> product
    3. summarize(product.value()) -> summary

    Use .value() to pass results between tasks in a type-safe way.

    Try: math_pipeline --a=20 --b=4
    """
    # Step 1: Divide
    quotient = divide(a=a, b=b)

    # Step 2: Multiply the result (use .value() to get the float)
    product = multiply(value=quotient.value(), factor=3)

    # Step 3: Summarize (use .value() to get the float)
    summarize(result=product.value())


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
    quotient = divide(a=a, b=b)
    product = multiply(value=quotient.value(), factor=5)
    summarize(result=product.value())


# =============================================================================
# CONDITIONAL EXECUTION WITH run_if
# =============================================================================
#
# Use run_if() to conditionally execute tasks based on flow parameters.
# This works both locally and in GitHub Actions workflows.


@recompose.task
def setup() -> recompose.Result[str]:
    """
    Initial setup step.

    """
    recompose.out("Running setup...")
    return recompose.Ok("setup-complete")


@recompose.task
def extra_validation() -> recompose.Result[str]:
    """
    Optional extra validation step.

    """
    recompose.out("Running extra validation...")
    return recompose.Ok("validation-passed")


@recompose.task
def finalize() -> recompose.Result[str]:
    """
    Final step.

    """
    recompose.out("Finalizing...")
    return recompose.Ok("done")


@recompose.flow
def conditional_pipeline(*, run_extra: bool = False) -> None:
    """
    A pipeline with conditional task execution.

    The run_if() context manager enables conditional execution:
    - Tasks inside run_if() only execute if the condition is true
    - Works identically in local execution and GitHub Actions
    - The condition becomes a separate evaluation step

    Try without extra validation:
        conditional_pipeline

    Try with extra validation:
        conditional_pipeline --run_extra

    Inspect to see the condition check step:
        inspect --target=conditional_pipeline

    IMPORTANT: Flows must have a STATIC task graph for GitHub Actions.
    You cannot use flow parameters in Python if/else statements:

        # WRONG - breaks GHA generation:
        if run_extra:
            extra_validation()

        # CORRECT - use run_if():
        with recompose.run_if(run_extra):
            extra_validation()

    """
    # Always runs
    setup()

    # Only runs if run_extra is True
    with recompose.run_if(run_extra):
        extra_validation()

    # Always runs
    finalize()


# =============================================================================
# ENTRYPOINT
# =============================================================================

if __name__ == "__main__":
    commands = [
        recompose.CommandGroup("Flows", [
            tool_check,
            greeting_pipeline,
            math_pipeline,
            risky_pipeline,
            conditional_pipeline,
        ]),
    ]
    recompose.main(commands=commands)
