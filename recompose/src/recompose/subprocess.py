"""Subprocess helpers for recompose tasks."""

from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path


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
        # Note: In tree mode, sys.stdout/stderr are already wrapped with PrefixWriter
        # which handles all the tree prefixing, so we just print normally here.

        proc = subprocess.Popen(
            cmd,
            cwd=cwd_str,
            env=run_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,  # Keep stderr separate for different formatting
            text=True,
            bufsize=1,  # Line buffered
        )

        stdout_lines: list[str] = []
        stderr_lines: list[str] = []

        def print_line(line: str, is_stderr: bool = False) -> None:
            """Print a line with appropriate formatting."""
            # Just print - the PrefixWriter on stdout/stderr handles tree prefixing
            if is_stderr:
                print(line, file=sys.stderr, flush=True)
            else:
                print(line, flush=True)

        # Stream output from both stdout and stderr
        # Use select on Unix, fallback to sequential reading on Windows
        if sys.platform != "win32" and proc.stdout and proc.stderr:
            # Unix: use select for interleaved output
            import selectors

            sel = selectors.DefaultSelector()
            sel.register(proc.stdout, selectors.EVENT_READ, ("stdout", stdout_lines))
            sel.register(proc.stderr, selectors.EVENT_READ, ("stderr", stderr_lines))

            while sel.get_map():
                for key, _ in sel.select():
                    stream_type, lines_list = key.data
                    line = key.fileobj.readline()  # type: ignore[union-attr]
                    if line:
                        line_stripped = line.rstrip("\n")
                        lines_list.append(line_stripped)
                        print_line(line_stripped, is_stderr=(stream_type == "stderr"))
                    else:
                        sel.unregister(key.fileobj)

            sel.close()
        else:
            # Windows or missing streams: read sequentially (stdout then stderr)
            if proc.stdout:
                for line in proc.stdout:
                    line_stripped = line.rstrip("\n")
                    stdout_lines.append(line_stripped)
                    print_line(line_stripped, is_stderr=False)
            if proc.stderr:
                for line in proc.stderr:
                    line_stripped = line.rstrip("\n")
                    stderr_lines.append(line_stripped)
                    print_line(line_stripped, is_stderr=True)

        proc.wait()

        result = RunResult(
            returncode=proc.returncode,
            stdout="\n".join(stdout_lines),
            stderr="\n".join(stderr_lines),
            command=cmd,
        )

    if check and result.failed:
        raise SubprocessError(result)

    return result
