#!/usr/bin/env python3
"""
Example demonstrating recompose subprocess helpers.

Run with:
    cd recompose
    uv run python examples/subprocess_demo.py check_repo
    uv run python examples/subprocess_demo.py list_files --path=/tmp
"""

import recompose


@recompose.task
def check_repo() -> recompose.Result[str]:
    """Check git repository status."""
    recompose.out("Checking git status...")

    # Run git status with captured output
    result = recompose.run("git", "status", "--porcelain", capture=True)

    if result.failed:
        return recompose.Err(f"git status failed: {result.stderr}")

    if result.stdout.strip():
        lines = result.stdout.strip().split("\n")
        recompose.out(f"Found {len(lines)} changed files:")
        for line in lines[:10]:  # Show first 10
            recompose.out(f"  {line}")
        if len(lines) > 10:
            recompose.out(f"  ... and {len(lines) - 10} more")
        return recompose.Ok(f"{len(lines)} files changed")
    else:
        recompose.out("Working directory is clean")
        return recompose.Ok("clean")


@recompose.task
def list_files(*, path: str = ".") -> recompose.Result[int]:
    """List files in a directory."""
    recompose.out(f"Listing files in {path}:")

    # Run ls with streaming output (shows in real-time)
    result = recompose.run("ls", "-la", path)

    if result.failed:
        return recompose.Err(f"ls failed with code {result.returncode}")

    return recompose.Ok(result.returncode)


@recompose.task
def run_python_version() -> recompose.Result[str]:
    """Show Python version."""
    result = recompose.run("python", "--version", capture=True)

    if result.ok:
        version = result.stdout.strip()
        recompose.out(f"Python version: {version}")
        return recompose.Ok(version)
    else:
        return recompose.Err("Failed to get Python version")


@recompose.task
def run_failing_command() -> recompose.Result[str]:
    """Demonstrate handling a failing command."""
    recompose.out("Running a command that will fail...")

    # This will fail (exit code 1)
    result = recompose.run("false")

    if result.failed:
        recompose.out(f"Command failed with exit code {result.returncode}")
        return recompose.Err("Command failed as expected")

    return recompose.Ok("unexpectedly succeeded")


@recompose.task
def run_with_check() -> recompose.Result[str]:
    """Demonstrate check=True behavior."""
    recompose.out("Running with check=True...")

    try:
        # This will raise SubprocessError
        recompose.run("false", check=True)
        return recompose.Ok("succeeded")
    except recompose.SubprocessError as e:
        recompose.out(f"Caught SubprocessError: {e}")
        return recompose.Err(str(e))


if __name__ == "__main__":
    recompose.main()
