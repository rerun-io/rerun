"""
Venv TaskClass for managing Python virtual environments.

This module provides the Venv TaskClass which encapsulates venv creation
and operations as a reusable component in flows.
"""

import shutil
import tempfile
from pathlib import Path

import recompose


@recompose.taskclass
class Venv:
    """
    A Python virtual environment manager.

    Encapsulates venv creation and package installation as a TaskClass
    that can be used in flows with proper state serialization.

    In flows:
        venv = Venv(location=some_path)  # Creates venv
        venv.install_wheel(wheel=wheel.value())  # Installs wheel

    When passed to other tasks, the task receives the Venv instance
    and can use its non-task methods like venv.python or venv.run().
    """

    def __init__(self, *, location: Path | None = None, python: str = "3.12", clean: bool = False):
        """
        Create a new virtual environment.

        Args:
            location: Path for the venv. If None, creates a temp directory.
            python: Python version to use.
            clean: If True, remove existing venv at location first.

        """
        if location is None:
            self.location = Path(tempfile.mkdtemp(prefix="recompose_venv_"))
        else:
            self.location = location
            if clean and self.location.exists():
                recompose.out(f"Cleaning existing venv at {self.location}...")
                shutil.rmtree(self.location)

        recompose.out(f"Creating venv at {self.location}...")

        result = recompose.run(
            "uv",
            "venv",
            str(self.location),
            "--python",
            python,
        )

        if result.failed:
            raise RuntimeError(f"Failed to create venv: {result.returncode}")

        recompose.out(f"Created venv: {self.location}")

    @property
    def python_path(self) -> Path:
        """Path to the venv's Python executable."""
        return self.location / "bin" / "python"

    @recompose.method
    def install_wheel(self, *, wheel: str, with_test_deps: bool = True) -> recompose.Result[None]:
        """
        Install a wheel into this virtual environment.

        Args:
            wheel: Path to the wheel file to install.
            with_test_deps: Also install pytest for running tests.

        """
        wheel_path = Path(wheel)

        if not wheel_path.exists():
            return recompose.Err(f"Wheel not found: {wheel_path}")

        if not self.python_path.exists():
            return recompose.Err(f"Venv python not found: {self.python_path}")

        recompose.out(f"Installing {wheel_path.name}...")

        result = recompose.run(
            "uv",
            "pip",
            "install",
            str(wheel_path),
            "--python",
            str(self.python_path),
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
                str(self.python_path),
            )
            if result.failed:
                return recompose.Err(f"Test deps installation failed: {result.returncode}")

        recompose.out("Installation complete!")
        return recompose.Ok(None)

    def run(self, *args: str, check: bool = True) -> recompose.RunResult:
        """
        Run a command in this venv.

        This is a regular method (not a task) that can be used
        when the Venv is passed to other tasks.

        Args:
            *args: Command arguments (first should be "python" or script path)
            check: If True, raise on non-zero exit (default: True)

        Returns:
            RunResult with stdout, stderr, and returncode.

        """
        # Prepend the venv's python to the command
        cmd = [str(self.python_path)] + list(args)
        return recompose.run(*cmd, check=check)
