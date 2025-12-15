#!/usr/bin/env python3
"""
Smoke test for the recompose package.

This script verifies that a recompose installation works correctly.
It's designed to be run against an installed package (not source)
to catch packaging issues.

Run directly: python smoke_test.py
Or via task: ./run smoke_test --venv=/path/to/venv
"""

import sys


def main() -> int:
    """Run smoke tests and return exit code."""
    print("Running recompose smoke tests...")

    # Test 1: Basic import
    print("  [1/5] Testing import...")
    try:
        import recompose
    except ImportError as e:
        print(f"  FAIL: Could not import recompose: {e}")
        return 1
    print(f"        recompose version: {recompose.__version__}")

    # Test 2: Result types
    print("  [2/5] Testing Result types...")
    ok_result = recompose.Ok("success")
    if not ok_result.ok:
        print("  FAIL: Ok result should have ok=True")
        return 1
    if ok_result.value != "success":
        print(f"  FAIL: Ok result value mismatch: {ok_result.value}")
        return 1

    err_result = recompose.Err("error message")
    if err_result.ok:
        print("  FAIL: Err result should have ok=False")
        return 1

    # Test 3: Task decorator
    print("  [3/5] Testing @task decorator...")

    @recompose.task
    def test_task(*, name: str) -> recompose.Result[str]:
        return recompose.Ok(f"Hello, {name}!")

    result = test_task(name="World")
    if not result.ok:
        print(f"  FAIL: Task returned error: {result}")
        return 1
    if result.value != "Hello, World!":
        print(f"  FAIL: Unexpected task result: {result.value}")
        return 1

    # Test 4: Subprocess helper
    print("  [4/5] Testing subprocess helper...")
    if not hasattr(recompose, "run"):
        print("  FAIL: Missing 'run' function")
        return 1

    run_result = recompose.run("echo", "test")
    if run_result.failed:
        print(f"  FAIL: Subprocess failed: {run_result}")
        return 1

    # Test 5: Flow decorator
    print("  [5/5] Testing flow decorator...")
    if not hasattr(recompose, "flow"):
        print("  FAIL: Missing 'flow' decorator")
        return 1

    print("\nAll smoke tests passed!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
