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
    uv run python -m examples.tutorial.intro_flows tool-check
    uv run python -m examples.tutorial.intro_flows greet-and-farewell --name="Alice"
    uv run python -m examples.tutorial.intro_flows math-pipeline --a=20 --b=4
    uv run python -m examples.tutorial.intro_flows conditional-pipeline
    uv run python -m examples.tutorial.intro_flows conditional-pipeline --run-extra
    uv run python -m examples.tutorial.intro_flows complex-conditional --run-extra --target=prod

Inspect flows without running:
    uv run python -m examples.tutorial.intro_flows inspect --target=tool_check
    uv run python -m examples.tutorial.intro_flows inspect --target=conditional_pipeline
"""

import recompose

# Import tasks from intro_tasks to compose into flows
from .intro_tasks import check_tool, divide, goodbye, hello

# =============================================================================
# ADDITIONAL TASKS FOR FLOWS
# =============================================================================
#
# These tasks are designed to be composed in flows.
# Notice the dependency parameters - they receive results from upstream tasks.


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
# FLOW WITH PARAMETERS AND DEPENDENCIES
# =============================================================================
#
# Flows can take parameters that are passed to tasks.
# Results from one task can be passed to another using .value()


@recompose.flow
def greet_and_farewell(*, name: str = "World") -> None:
    """
    A pipeline that greets and then says farewell.

    Flow parameters become CLI options:
        greet_and_farewell --name="Alice"

    Tasks are wired together using .value() to pass results:
        greeting = hello(name=name)              # Returns Result[str]
        goodbye(greeting=greeting.value(), ...)  # .value() gives str

    Note: hello() randomly picks a greeting (Hello, Hi, or Hey).
    goodbye() only knows farewells for "Hello" and "Hi", so
    if hello() returns "Hey", the flow will fail at goodbye().

    Try it several times to see both success and failure cases!
    """
    # First, generate a random greeting
    greeting = hello(name=name)

    # Then generate a farewell based on the greeting
    # This may fail if hello() returned "Hey" (unknown farewell)
    goodbye(greeting=greeting.value(), name=name)


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
# COMPLEX CONDITIONAL EXPRESSIONS
# =============================================================================
#
# run_if() supports complex boolean expressions using Python operators:
#   - & (and): run_extra & (target == "prod")
#   - | (or): deploy | (env == "staging")
#   - ~ (not): ~skip_tests
#   - == / != : target == "prod"


@recompose.task
def deploy_to_prod() -> recompose.Result[str]:
    """Deploy to production environment."""
    recompose.out("Deploying to production...")
    return recompose.Ok("deployed-to-prod")


@recompose.flow
def complex_conditional(*, run_extra: bool = False, target: str = "staging") -> None:
    """
    A pipeline with complex conditional expressions.

    The conditional task runs only if BOTH conditions are true:
        run_extra AND (target == "prod")

    This demonstrates combining boolean inputs with string comparisons.

    Try different combinations:
        # Won't deploy (run_extra=False):
        complex_conditional

        # Won't deploy (target != "prod"):
        complex_conditional --run_extra

        # WILL deploy (both conditions met):
        complex_conditional --run_extra --target=prod

    In the generated GHA workflow, the condition becomes:
        - name: run_if_1
          id: run_if_1
          run: |
            # [if: run_extra and target == prod]
            uv run python -m ... --step run_if_1

        - name: step_N_deploy_to_prod
          if: ${{ steps.run_if_1.outputs.value == 'true' }}
          run: ...

    """
    # Always runs
    setup()

    # Only runs if run_extra AND target == "prod"
    # Use & for 'and' (Python's 'and' keyword won't work with expressions)
    with recompose.run_if(run_extra & (target == "prod")):
        deploy_to_prod()

    # Always runs
    finalize()


# =============================================================================
# ENTRYPOINT
# =============================================================================

if __name__ == "__main__":
    commands = [
        recompose.CommandGroup(
            "Flows",
            [
                tool_check,
                greet_and_farewell,
                math_pipeline,
                risky_pipeline,
                conditional_pipeline,
                complex_conditional,
            ],
        ),
    ]
    recompose.main(commands=commands)
