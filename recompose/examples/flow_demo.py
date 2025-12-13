#!/usr/bin/env python3
"""
Example demonstrating recompose flows.

Flows compose multiple tasks into a pipeline. Each task execution is tracked.

Recompose supports two flow styles:
1. Imperative (legacy): Call tasks directly, they execute immediately
2. Declarative: Use task.flow() to build a graph, then execute

Run with:
    cd recompose
    uv run python examples/flow_demo.py --help
    uv run python examples/flow_demo.py build_and_test
    uv run python examples/flow_demo.py build_and_test --skip_tests
    uv run python examples/flow_demo.py declarative_pipeline
"""

import time

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

    time.sleep(0.05)
    return recompose.Ok(None)


@recompose.task
def run_linter() -> recompose.Result[None]:
    """Run the linter (simulated)."""
    recompose.out("Running linter...")
    time.sleep(0.08)
    recompose.out("  No lint errors found")
    return recompose.Ok(None)


@recompose.task
def run_type_checker() -> recompose.Result[None]:
    """Run the type checker (simulated)."""
    recompose.out("Running type checker...")
    time.sleep(0.12)
    recompose.out("  All types check out")
    return recompose.Ok(None)


@recompose.task
def run_tests() -> recompose.Result[int]:
    """Run tests (simulated)."""
    recompose.out("Running tests...")
    time.sleep(0.25)
    recompose.out("  10 tests passed")
    return recompose.Ok(10)


@recompose.task
def build_artifact(*, output: str = "build/app") -> recompose.Result[str]:
    """Build the artifact (simulated)."""
    recompose.out(f"Building artifact to {output}...")
    time.sleep(0.15)
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

    If any task fails, the flow automatically stops and returns that failure.
    """
    # Check prerequisites first
    check_prerequisites()

    # Run quality checks
    run_linter()
    run_type_checker()

    # Run tests unless skipped
    if not skip_tests:
        tests = run_tests()
        recompose.out(f"  {tests.value} tests passed!")

    # Build the artifact
    build = build_artifact()

    return recompose.Ok(f"Pipeline complete! Artifact: {build.value}")


@recompose.flow
def quick_check() -> recompose.Result[None]:
    """Quick check - just lint and type check."""
    run_linter()
    run_type_checker()
    recompose.out("Quick check passed!")
    return recompose.Ok(None)


@recompose.task
def failing_lint() -> recompose.Result[None]:
    """A linter that always fails (for demo)."""
    recompose.out("Running strict linter...")
    recompose.out("  ERROR: Found 3 lint errors")
    return recompose.Err("Lint check failed: 3 errors")


@recompose.flow
def strict_check() -> recompose.Result[None]:
    """
    Strict check that will fail.

    Demonstrates automatic flow failure when a task fails.
    """
    recompose.out("Running strict checks...")
    failing_lint()  # This will fail and stop the flow
    run_type_checker()  # This won't run
    return recompose.Ok(None)


# You can also have standalone tasks alongside flows
@recompose.task
def clean() -> recompose.Result[None]:
    """Clean build artifacts (simulated)."""
    recompose.out("Cleaning build artifacts...")
    recompose.out("  Done")
    return recompose.Ok(None)


# ============================================================================
# DECLARATIVE FLOWS (P05b) - New API
# ============================================================================
#
# Declarative flows use task.flow() to build a task graph before execution.
# This enables:
# - Dry-run / plan inspection
# - Clear dependency tracking
# - Future: parallel execution, subprocess isolation, GHA generation


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
def package_artifact(*, binary: str, test_count: int) -> recompose.Result[str]:
    """Package the binary into a distributable artifact."""
    recompose.out(f"Packaging {binary} (verified with {test_count} tests)...")
    time.sleep(0.05)
    return recompose.Ok("/tmp/dist/app.tar.gz")


@recompose.flow
def declarative_pipeline(*, repo: str = "main"):
    """
    Declarative build pipeline using task.flow().

    This flow builds a task graph and then executes it:
    1. fetch_source
    2. compile_source (depends on fetch_source)
    3. run_unit_tests (depends on compile)
    4. run_integration_tests (depends on compile, can run parallel to unit tests)
    5. package_artifact (depends on compile and unit_tests)

    Try: uv run python examples/flow_demo.py declarative_pipeline
    """
    # Build the task graph using .flow()
    source = fetch_source.flow(repo=repo)
    binary = compile_source.flow(source_dir=source)  # Depends on source

    # These could run in parallel (both depend only on binary)
    unit_tests = run_unit_tests.flow(binary=binary)
    integration_tests = run_integration_tests.flow(binary=binary)

    # Package depends on binary and unit test count
    package = package_artifact.flow(binary=binary, test_count=unit_tests)

    return package  # Terminal node


@recompose.flow
def show_plan_demo():
    """
    Demonstrate the plan() feature for dry-run inspection.

    This flow shows how to inspect the execution plan before running.
    """
    # Get the plan without executing
    plan = declarative_pipeline.plan(repo="feature-branch")

    recompose.out("=== Flow Plan ===")
    recompose.out(f"Total tasks: {len(plan.nodes)}")
    recompose.out(f"Terminal task: {plan.terminal.name if plan.terminal else 'None'}")
    recompose.out("")

    recompose.out("Execution order:")
    for i, node in enumerate(plan.get_execution_order(), 1):
        deps = [d.name for d in node.dependencies]
        dep_str = f" <- {deps}" if deps else ""
        recompose.out(f"  {i}. {node.name}{dep_str}")

    recompose.out("")
    recompose.out("Parallelizable groups:")
    for level, group in enumerate(plan.get_parallelizable_groups()):
        names = [n.name for n in group]
        recompose.out(f"  Level {level}: {', '.join(names)}")

    recompose.out("")
    recompose.out("Graph visualization:")
    recompose.out(plan.visualize())

    return recompose.Ok(None)


if __name__ == "__main__":
    recompose.main()
