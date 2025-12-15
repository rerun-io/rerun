"""
Build and distribution tasks for the recompose project.

These tasks handle building wheels, creating test environments,
and validating that the installed package works correctly.
"""

import shutil
import tempfile
from pathlib import Path

import recompose

# Project root is two levels up from tasks/
PROJECT_ROOT = Path(__file__).parent.parent.parent


@recompose.task
def build_wheel(*, output_dir: str | None = None) -> recompose.Result[str]:
    """
    Build a wheel distribution of the recompose package.

    Args:
        output_dir: Directory to place the wheel. Defaults to dist/ in project root.

    Returns:
        Path to the built wheel file as a string.
    """
    if output_dir is None:
        dist_dir = PROJECT_ROOT / "dist"
    else:
        dist_dir = Path(output_dir)

    # Clean the output directory
    if dist_dir.exists():
        recompose.out(f"Cleaning {dist_dir}...")
        shutil.rmtree(dist_dir)

    recompose.out(f"Building wheel to {dist_dir}...")

    result = recompose.run(
        "uv",
        "build",
        "--wheel",
        "--out-dir",
        str(dist_dir),
        cwd=PROJECT_ROOT,
    )

    if result.failed:
        return recompose.Err(f"Build failed with exit code {result.returncode}")

    # Find the built wheel
    wheels = list(dist_dir.glob("*.whl"))
    if not wheels:
        return recompose.Err(f"No wheel found in {dist_dir}")

    if len(wheels) > 1:
        return recompose.Err(f"Multiple wheels found in {dist_dir}: {wheels}")

    wheel_path = wheels[0]
    recompose.out(f"Built: {wheel_path.name}")
    return recompose.Ok(str(wheel_path))


@recompose.task
def create_test_venv(*, location: str | None = None) -> recompose.Result[str]:
    """
    Create an isolated virtual environment for testing.

    Args:
        location: Path for the venv. If None, creates a temp directory.

    Returns:
        Path to the created venv as a string.
    """
    if location is None:
        # Create a temp directory that persists until explicitly cleaned
        venv_path = Path(tempfile.mkdtemp(prefix="recompose_test_"))
    else:
        venv_path = Path(location)
        if venv_path.exists():
            recompose.out(f"Cleaning existing venv at {venv_path}...")
            shutil.rmtree(venv_path)

    recompose.out(f"Creating test venv at {venv_path}...")

    result = recompose.run(
        "uv",
        "venv",
        str(venv_path),
        "--python",
        "3.12",
    )

    if result.failed:
        return recompose.Err(f"Failed to create venv: {result.returncode}")

    recompose.out(f"Created venv: {venv_path}")
    return recompose.Ok(str(venv_path))


@recompose.task
def install_wheel(*, wheel: str, venv: str, with_test_deps: bool = True) -> recompose.Result[None]:
    """
    Install a wheel into a virtual environment.

    Args:
        wheel: Path to the wheel file to install.
        venv: Path to the virtual environment.
        with_test_deps: Also install pytest for running tests.
    """
    wheel_path = Path(wheel)
    venv_path = Path(venv)
    python = venv_path / "bin" / "python"

    if not wheel_path.exists():
        return recompose.Err(f"Wheel not found: {wheel_path}")

    if not python.exists():
        return recompose.Err(f"Venv python not found: {python}")

    recompose.out(f"Installing {wheel_path.name} into {venv_path.name}...")

    result = recompose.run(
        "uv",
        "pip",
        "install",
        str(wheel_path),
        "--python",
        str(python),
    )

    if result.failed:
        return recompose.Err(f"Installation failed: {result.returncode}")

    if with_test_deps:
        recompose.out("Installing test dependencies (pytest)...")
        result = recompose.run(
            "uv",
            "pip",
            "install",
            "pytest",
            "--python",
            str(python),
        )
        if result.failed:
            return recompose.Err(f"Test deps installation failed: {result.returncode}")

    recompose.out("Installation complete!")
    return recompose.Ok(None)


@recompose.task
def smoke_test(*, venv: str) -> recompose.Result[None]:
    """
    Run a quick smoke test on the installed package.

    Runs examples/tasks/smoke_test.py using the venv's python
    to verify basic import and functionality.

    Args:
        venv: Path to the virtual environment with recompose installed.
    """
    venv_path = Path(venv)
    python = venv_path / "bin" / "python"
    smoke_test_script = Path(__file__).parent / "smoke_test.py"

    if not python.exists():
        return recompose.Err(f"Python not found in venv: {python}")

    if not smoke_test_script.exists():
        return recompose.Err(f"Smoke test script not found: {smoke_test_script}")

    recompose.out("Running smoke test...")

    result = recompose.run(
        str(python),
        str(smoke_test_script),
    )

    if result.failed:
        return recompose.Err(f"Smoke test failed: {result.returncode}")

    recompose.out("Smoke test passed!")
    return recompose.Ok(None)


@recompose.task
def test_installed(*, venv: str, verbose: bool = False) -> recompose.Result[None]:
    """
    Run the full test suite against the installed package.

    Uses the venv's python to run pytest, ensuring tests run against
    the installed package rather than source.

    Args:
        venv: Path to the virtual environment with recompose installed.
        verbose: Show verbose test output.
    """
    venv_path = Path(venv)
    python = venv_path / "bin" / "python"
    tests_dir = PROJECT_ROOT / "tests"

    if not python.exists():
        return recompose.Err(f"Python not found in venv: {python}")

    if not tests_dir.exists():
        return recompose.Err(f"Tests directory not found: {tests_dir}")

    recompose.out(f"Running tests from {tests_dir} using installed package...")

    args = [str(python), "-m", "pytest", str(tests_dir)]
    if verbose:
        args.append("-v")

    result = recompose.run(*args)

    if result.failed:
        return recompose.Err(f"Tests failed: {result.returncode}")

    recompose.out("All tests passed against installed package!")
    return recompose.Ok(None)
