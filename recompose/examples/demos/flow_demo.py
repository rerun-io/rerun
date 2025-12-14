#!/usr/bin/env python3
"""
Example demonstrating recompose flows.

Flows compose multiple tasks into a dependency graph using task.flow() calls.
The flow builds the graph first, then executes tasks in topological order.

Run with:
    cd recompose
    uv run python examples/flow_demo.py --help
    uv run python examples/flow_demo.py build_pipeline
    uv run python examples/flow_demo.py build_pipeline --repo=feature-branch
    uv run python examples/flow_demo.py quality_check

Inspect flows without executing:
    uv run python examples/flow_demo.py inspect build_pipeline
    uv run python examples/flow_demo.py inspect quality_check
"""

import time

import recompose

# ============================================================================
# TASKS
# ============================================================================


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

    time.sleep(0.05)
    return recompose.Ok(None)


@recompose.task
def run_linter(*, prereq: None = None) -> recompose.Result[None]:
    """Run the linter (simulated)."""
    recompose.out("Running linter...")
    time.sleep(0.08)
    recompose.out("  No lint errors found")
    return recompose.Ok(None)


@recompose.task
def run_type_checker(*, prereq: None = None) -> recompose.Result[None]:
    """Run the type checker (simulated)."""
    recompose.out("Running type checker...")
    time.sleep(0.12)
    recompose.out("  All types check out")
    return recompose.Ok(None)


@recompose.task
def run_tests(*, lint_ok: None, types_ok: None) -> recompose.Result[int]:
    """Run tests (simulated)."""
    recompose.out("Running tests...")
    time.sleep(0.25)
    recompose.out("  10 tests passed")
    return recompose.Ok(10)


@recompose.task
def build_artifact(*, test_count: int, output: str = "build/app") -> recompose.Result[str]:
    """Build the artifact (simulated)."""
    recompose.out(f"Building artifact to {output}...")
    recompose.out(f"  Verified with {test_count} tests")
    time.sleep(0.15)
    recompose.out("  Build complete")
    return recompose.Ok(output)


@recompose.task
def failing_lint() -> recompose.Result[None]:
    """A linter that always fails (for demo)."""
    recompose.out("Running strict linter...")
    recompose.out("  ERROR: Found 3 lint errors")
    return recompose.Err("Lint check failed: 3 errors")


@recompose.task
def clean() -> recompose.Result[None]:
    """Clean build artifacts (simulated)."""
    recompose.out("Cleaning build artifacts...")
    recompose.out("  Done")
    return recompose.Ok(None)


@recompose.task
def fetch_source(*, repo: str = "main") -> recompose.Result[str]:
    """Fetch source code from repository."""
    recompose.out(f"Fetching source from {repo}...")
    time.sleep(0.05)
    return recompose.Ok(f"/tmp/src/{repo}")


@recompose.task
def compile_source(*, source_dir: str) -> recompose.Result[str]:
    """Compile the source code."""
    recompose.out(f"Compiling {source_dir}...")
    time.sleep(0.1)
    return recompose.Ok(f"{source_dir}/build/output.bin")


@recompose.task
def run_unit_tests(*, binary: str) -> recompose.Result[int]:
    """Run unit tests on the compiled binary."""
    recompose.out(f"Testing {binary}...")
    time.sleep(0.15)
    recompose.out("  All 42 unit tests passed")
    return recompose.Ok(42)


@recompose.task
def run_integration_tests(*, binary: str) -> recompose.Result[int]:
    """Run integration tests on the compiled binary."""
    recompose.out(f"Integration testing {binary}...")
    time.sleep(0.2)
    recompose.out("  All 12 integration tests passed")
    return recompose.Ok(12)


@recompose.task
def package_artifact(*, binary: str, unit_tests: int, integration_tests: int) -> recompose.Result[str]:
    """Package the binary into a distributable artifact."""
    total_tests = unit_tests + integration_tests
    recompose.out(f"Packaging {binary} (verified with {total_tests} tests)...")
    time.sleep(0.05)
    return recompose.Ok("/tmp/dist/app.tar.gz")


# ============================================================================
# FLOWS
# ============================================================================
#
# Flows use task.flow() to build a task graph before execution.
# This enables:
# - Dry-run / plan inspection via flow.plan()
# - Clear dependency tracking
# - Future: parallel execution, subprocess isolation, GHA generation


@recompose.task
def quality_gate(*, lint_ok: None, types_ok: None) -> recompose.Result[None]:
    """Gate that waits for lint and type check to complete."""
    recompose.out("Quality checks passed!")
    return recompose.Ok(None)


@recompose.flow
def quality_check() -> None:
    """
    Quick quality check - lint and type check in parallel.

    Try: uv run python examples/flow_demo.py quality_check
    """
    prereq = check_prerequisites.flow()
    lint = run_linter.flow(prereq=prereq)
    types = run_type_checker.flow(prereq=prereq)
    # Both lint and types must complete before quality_gate
    quality_gate.flow(lint_ok=lint, types_ok=types)


@recompose.flow
def build_and_test() -> None:
    """
    Full build and test pipeline.

    This flow:
    1. Checks prerequisites
    2. Runs linter and type checker (can run in parallel)
    3. Runs tests (depends on lint and types)
    4. Builds artifact (depends on tests)

    Try: uv run python examples/flow_demo.py build_and_test
    """
    prereq = check_prerequisites.flow()
    lint = run_linter.flow(prereq=prereq)
    types = run_type_checker.flow(prereq=prereq)
    tests = run_tests.flow(lint_ok=lint, types_ok=types)
    build_artifact.flow(test_count=tests)


@recompose.flow
def strict_check() -> None:
    """
    Strict check that will fail.

    Demonstrates automatic flow failure when a task fails.

    Try: uv run python examples/flow_demo.py strict_check
    """
    lint = failing_lint.flow()  # This will fail
    run_type_checker.flow(prereq=lint)  # Won't run


@recompose.flow
def build_pipeline(*, repo: str = "main") -> None:
    """
    Full build pipeline with explicit dependencies.

    This flow builds a task graph and then executes it:
    1. fetch_source
    2. compile_source (depends on fetch_source)
    3. run_unit_tests (depends on compile)
    4. run_integration_tests (depends on compile, can run parallel to unit tests)
    5. package_artifact (depends on all tests passing)

    Try: uv run python examples/flow_demo.py build_pipeline
         uv run python examples/flow_demo.py build_pipeline --repo=feature-branch
    """
    source = fetch_source.flow(repo=repo)
    binary = compile_source.flow(source_dir=source)

    # These run in parallel (both depend only on binary)
    unit_tests = run_unit_tests.flow(binary=binary)
    integration_tests = run_integration_tests.flow(binary=binary)

    # Package depends on all tests completing
    package_artifact.flow(binary=binary, unit_tests=unit_tests, integration_tests=integration_tests)


if __name__ == "__main__":
    recompose.main()
