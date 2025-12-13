#!/usr/bin/env python3
"""
Example demonstrating recompose flows.

Flows compose multiple tasks into a pipeline. Each task execution is tracked.

Run with:
    cd recompose
    uv run python examples/flow_demo.py --help
    uv run python examples/flow_demo.py build_and_test
    uv run python examples/flow_demo.py build_and_test --skip_tests
"""

import recompose


@recompose.task
def check_prerequisites() -> recompose.Result[None]:
    """Check that required tools are available."""
    recompose.out("Checking prerequisites...")

    # Check for git
    result = recompose.run("git", "--version", capture=True)
    if result.failed:
        return recompose.Err("git not found")
    recompose.out(f"  Found {result.stdout.strip()}")

    # Check for python
    result = recompose.run("python", "--version", capture=True)
    if result.failed:
        return recompose.Err("python not found")
    recompose.out(f"  Found {result.stdout.strip()}")

    return recompose.Ok(None)


@recompose.task
def run_linter() -> recompose.Result[None]:
    """Run the linter (simulated)."""
    recompose.out("Running linter...")
    recompose.out("  No lint errors found")
    return recompose.Ok(None)


@recompose.task
def run_type_checker() -> recompose.Result[None]:
    """Run the type checker (simulated)."""
    recompose.out("Running type checker...")
    recompose.out("  All types check out")
    return recompose.Ok(None)


@recompose.task
def run_tests() -> recompose.Result[int]:
    """Run tests (simulated)."""
    recompose.out("Running tests...")
    recompose.out("  10 tests passed")
    return recompose.Ok(10)


@recompose.task
def build_artifact(*, output: str = "build/app") -> recompose.Result[str]:
    """Build the artifact (simulated)."""
    recompose.out(f"Building artifact to {output}...")
    recompose.out("  Build complete")
    return recompose.Ok(output)


@recompose.flow
def build_and_test(*, skip_tests: bool = False) -> recompose.Result[str]:
    """
    Full build and test pipeline.

    This flow:
    1. Checks prerequisites
    2. Runs linter
    3. Runs type checker
    4. Runs tests (optional)
    5. Builds artifact
    """
    # Check prerequisites first
    prereq = check_prerequisites()
    if prereq.failed:
        return prereq

    # Run quality checks
    lint = run_linter()
    if lint.failed:
        return lint

    types = run_type_checker()
    if types.failed:
        return types

    # Run tests unless skipped
    if not skip_tests:
        tests = run_tests()
        if tests.failed:
            return tests
        recompose.out(f"  {tests.value} tests passed!")

    # Build the artifact
    build = build_artifact()
    if build.failed:
        return build

    return recompose.Ok(f"Pipeline complete! Artifact: {build.value}")


@recompose.flow
def quick_check() -> recompose.Result[None]:
    """Quick check - just lint and type check."""
    lint = run_linter()
    if lint.failed:
        return lint

    types = run_type_checker()
    if types.failed:
        return types

    recompose.out("Quick check passed!")
    return recompose.Ok(None)


# You can also have standalone tasks alongside flows
@recompose.task
def clean() -> recompose.Result[None]:
    """Clean build artifacts (simulated)."""
    recompose.out("Cleaning build artifacts...")
    recompose.out("  Done")
    return recompose.Ok(None)


if __name__ == "__main__":
    recompose.main()
