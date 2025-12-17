#!/usr/bin/env python3
"""
Tutorial: Introduction to Recompose Tasks

This tutorial introduces the core concepts of recompose:
- The @task decorator
- Result types: Ok and Err
- CLI argument generation from function signatures
- Output helpers: recompose.out() and recompose.dbg()
- Subprocess execution with recompose.run()

Run this file to see all tasks:
    uv run python examples/tutorial/intro_tasks.py --help

Try individual tasks:
    uv run python examples/tutorial/intro_tasks.py hello
    uv run python examples/tutorial/intro_tasks.py greet --name="Alice"
    uv run python examples/tutorial/intro_tasks.py check_tool --tool=git
    uv run python examples/tutorial/intro_tasks.py divide --a=10 --b=2
    uv run python examples/tutorial/intro_tasks.py divide --a=10 --b=0
"""

import recompose

# =============================================================================
# BASIC TASKS
# =============================================================================
#
# A task is just a function decorated with @recompose.task.
# Tasks return Result[T] using Ok(value) for success or Err(message) for failure.


@recompose.task
def hello() -> recompose.Result[str]:
    """
    The simplest possible task.

    Returns a greeting message. This demonstrates:
    - The @task decorator
    - Returning Ok(value) for success
    """
    return recompose.Ok("Hello from recompose!")


@recompose.task
def greet(*, name: str = "World") -> recompose.Result[str]:
    """
    A task with CLI arguments.

    Function parameters become CLI options automatically:
    - Keyword-only args (after *) become --name=value options
    - Default values make arguments optional
    - Type hints determine argument types

    Try: greet --name="Alice"
    """
    message = f"Hello, {name}!"
    recompose.out(message)  # Output to console (captured by recompose)
    return recompose.Ok(message)


# =============================================================================
# OUTPUT HELPERS
# =============================================================================
#
# recompose.out() - Standard output, always shown
# recompose.dbg() - Debug output, only shown with --debug flag


@recompose.task
def verbose_task(*, count: int = 3) -> recompose.Result[int]:
    """
    Demonstrates output helpers.

    - recompose.out() prints to console (always visible)
    - recompose.dbg() prints debug info (only with --debug flag)

    Try: verbose_task --count=5
    Try: verbose_task --count=5 --debug
    """
    recompose.dbg(f"Starting with count={count}")

    for i in range(count):
        recompose.dbg(f"  Iteration {i + 1}")
        recompose.out(f"Processing item {i + 1} of {count}")

    recompose.dbg("Done!")
    return recompose.Ok(count)


# =============================================================================
# SUBPROCESS EXECUTION
# =============================================================================
#
# recompose.run() executes shell commands with proper output handling.


@recompose.task
def check_tool(*, tool: str = "git") -> recompose.Result[str]:
    """
    Check if a command-line tool is available.

    Demonstrates recompose.run() for subprocess execution:
    - capture=True captures stdout/stderr instead of streaming
    - result.ok / result.failed check exit status
    - result.stdout / result.stderr access captured output

    Try: check_tool --tool=git
    Try: check_tool --tool=nonexistent
    """
    recompose.out(f"Checking for {tool}...")

    # Run with capture=True to get output as strings
    result = recompose.run(tool, "--version", capture=True)

    if result.failed:
        recompose.out(f"  {tool} not found!")
        return recompose.Err(f"{tool} is not available")

    version = result.stdout.strip()
    recompose.out(f"  Found: {version}")
    return recompose.Ok(version)


@recompose.task
def list_files(*, path: str = ".") -> recompose.Result[int]:
    """
    List files in a directory.

    Demonstrates recompose.run() with streaming output:
    - Without capture=True, output streams to console in real-time
    - Good for long-running commands where you want to see progress

    Try: list_files --path=/tmp
    """
    recompose.out(f"Listing files in {path}:")

    # Without capture=True, output streams directly to console
    result = recompose.run("ls", "-la", path)

    if result.failed:
        return recompose.Err(f"ls failed with code {result.returncode}")

    return recompose.Ok(result.returncode)


# =============================================================================
# ERROR HANDLING
# =============================================================================
#
# Tasks return Err(message) to indicate failure.
# The @task decorator also catches uncaught exceptions automatically.


@recompose.task
def divide(*, a: int, b: int) -> recompose.Result[float]:
    """
    Divide two numbers, demonstrating error handling.

    Returns Err when division by zero is attempted.

    Try: divide --a=10 --b=2
    Try: divide --a=10 --b=0
    """
    if b == 0:
        return recompose.Err("Cannot divide by zero")

    result = a / b
    recompose.out(f"{a} / {b} = {result}")
    return recompose.Ok(result)


@recompose.task
def might_crash(*, should_crash: bool = False) -> recompose.Result[str]:
    """
    Demonstrates automatic exception handling.

    The @task decorator catches uncaught exceptions and converts them
    to Err results. You don't need try/except unless you want custom
    error handling.

    Try: might_crash
    Try: might_crash --should_crash
    """
    if should_crash:
        raise ValueError("This is an intentional crash!")

    return recompose.Ok("No crash occurred")


# =============================================================================
# ENTRYPOINT
# =============================================================================

if __name__ == "__main__":
    commands = [
        recompose.CommandGroup("Examples", [
            hello,
            greet,
            verbose_task,
            check_tool,
            list_files,
            divide,
            might_crash,
        ]),
    ]
    recompose.main(commands=commands)
