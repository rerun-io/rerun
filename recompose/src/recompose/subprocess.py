"""Subprocess helpers for recompose tasks."""

from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

from .context import get_context, out


@dataclass
class RunResult:
    """
    Result from running a subprocess.

    Attributes:
        returncode: The exit code of the process.
        stdout: Captured stdout (empty string if streaming).
        stderr: Captured stderr (empty string if streaming).
        command: The command that was executed.

    """

    returncode: int
    stdout: str = ""
    stderr: str = ""
    command: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        """True if the command succeeded (exit code 0)."""
        return self.returncode == 0

    @property
    def failed(self) -> bool:
        """True if the command failed (non-zero exit code)."""
        return self.returncode != 0


class SubprocessError(Exception):
    """Raised when a subprocess fails and check=True."""

    def __init__(self, result: RunResult):
        self.result = result
        cmd_str = " ".join(result.command)
        super().__init__(f"Command '{cmd_str}' failed with exit code {result.returncode}")


def run(
    *args: str | Path,
    cwd: str | Path | None = None,
    env: dict[str, str] | None = None,
    capture: bool = False,
    check: bool = False,
) -> RunResult:
    """
    Run a subprocess command.

    By default, output is streamed to the console in real-time.
    Use `capture=True` to capture output for parsing instead.

    Args:
        *args: Command and arguments to run (e.g., "cargo", "build", "--release")
        cwd: Working directory for the command
        env: Additional environment variables (merged with current environment)
        capture: If True, capture stdout/stderr instead of streaming
        check: If True, raise SubprocessError on non-zero exit code

    Returns:
        RunResult with exit code and captured output (if capture=True)

    Raises:
        SubprocessError: If check=True and the command fails
        FileNotFoundError: If the command is not found

    Example:
        >>> result = run("echo", "hello")
        hello
        >>> result.ok
        True

        >>> result = run("git", "status", "--porcelain", capture=True)
        >>> if result.stdout:
        ...     print("Working directory has changes")

    """
    # Convert Path objects to strings
    cmd = [str(arg) for arg in args]

    # Build environment
    run_env = os.environ.copy()
    if env:
        run_env.update(env)

    # Convert cwd to string if needed
    cwd_str = str(cwd) if cwd else None

    if capture:
        # Capture mode - collect all output
        completed = subprocess.run(
            cmd,
            cwd=cwd_str,
            env=run_env,
            capture_output=True,
            text=True,
        )
        result = RunResult(
            returncode=completed.returncode,
            stdout=completed.stdout,
            stderr=completed.stderr,
            command=cmd,
        )
    else:
        # Streaming mode - output goes to console in real-time
        # We use Popen to have more control over output handling
        proc = subprocess.Popen(
            cmd,
            cwd=cwd_str,
            env=run_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,  # Merge stderr into stdout for simpler streaming
            text=True,
            bufsize=1,  # Line buffered
        )

        stdout_lines: list[str] = []
        ctx = get_context()

        # Stream output line by line
        if proc.stdout:
            for line in proc.stdout:
                line_stripped = line.rstrip("\n")
                stdout_lines.append(line_stripped)
                # Use recompose.out() if in task context, otherwise print directly
                if ctx is not None:
                    out(line_stripped)
                else:
                    print(line_stripped, file=sys.stdout, flush=True)

        proc.wait()

        result = RunResult(
            returncode=proc.returncode,
            stdout="\n".join(stdout_lines),
            stderr="",  # Merged into stdout in streaming mode
            command=cmd,
        )

    if check and result.failed:
        raise SubprocessError(result)

    return result
