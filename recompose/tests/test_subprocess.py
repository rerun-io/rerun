"""Tests for subprocess helpers."""

import tempfile
from pathlib import Path

import pytest

from recompose.subprocess import RunResult, SubprocessError, run


def test_run_simple_command():
    """Test running a simple command."""
    result = run("echo", "hello", capture=True)
    assert result.ok
    assert result.returncode == 0
    assert result.stdout.strip() == "hello"


def test_run_result_properties():
    """Test RunResult ok/failed properties."""
    success = RunResult(returncode=0, command=["test"])
    assert success.ok
    assert not success.failed

    failure = RunResult(returncode=1, command=["test"])
    assert not failure.ok
    assert failure.failed


def test_run_with_arguments():
    """Test running command with multiple arguments."""
    result = run("echo", "hello", "world", capture=True)
    assert result.ok
    assert result.stdout.strip() == "hello world"


def test_run_failing_command():
    """Test that failing commands return non-zero exit code."""
    result = run("false", capture=True)  # 'false' command always exits with 1
    assert result.failed
    assert result.returncode != 0


def test_run_with_check_raises():
    """Test that check=True raises on failure."""
    with pytest.raises(SubprocessError) as exc_info:
        run("false", check=True)

    assert exc_info.value.result.returncode != 0
    assert "false" in str(exc_info.value)


def test_run_with_check_success():
    """Test that check=True doesn't raise on success."""
    result = run("true", check=True, capture=True)
    assert result.ok


def test_run_with_cwd():
    """Test running command in a different directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a file in the temp dir
        test_file = Path(tmpdir) / "test.txt"
        test_file.write_text("content")

        # List files in that directory
        result = run("ls", cwd=tmpdir, capture=True)
        assert result.ok
        assert "test.txt" in result.stdout


def test_run_with_env():
    """Test running command with custom environment variables."""
    result = run("sh", "-c", "echo $MY_TEST_VAR", env={"MY_TEST_VAR": "hello123"}, capture=True)
    assert result.ok
    assert "hello123" in result.stdout


def test_run_env_merges_with_existing():
    """Test that env vars are merged, not replaced."""
    # PATH should still be available even when adding custom vars
    result = run("sh", "-c", "echo $PATH", env={"MY_VAR": "test"}, capture=True)
    assert result.ok
    assert result.stdout.strip()  # PATH should not be empty


def test_run_captures_stderr():
    """Test that stderr is captured in capture mode."""
    result = run("sh", "-c", "echo error >&2", capture=True)
    assert result.ok
    assert "error" in result.stderr


def test_run_command_stored_in_result():
    """Test that the command is stored in the result."""
    result = run("echo", "test", capture=True)
    assert result.command == ["echo", "test"]


def test_run_with_path_objects():
    """Test that Path objects work for arguments."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmppath = Path(tmpdir)
        result = run("ls", tmppath, capture=True)
        assert result.ok


def test_run_streaming_mode(capsys):
    """Test that streaming mode outputs to console."""
    result = run("echo", "streamed output")
    assert result.ok
    # In streaming mode, output should have been printed
    captured = capsys.readouterr()
    assert "streamed output" in captured.out


def test_run_not_found():
    """Test running a command that doesn't exist."""
    with pytest.raises(FileNotFoundError):
        run("nonexistent_command_12345", capture=True)


def test_subprocess_error_message():
    """Test SubprocessError has informative message."""
    result = RunResult(returncode=1, command=["my", "command"])
    error = SubprocessError(result)
    assert "my command" in str(error)
    assert "exit code 1" in str(error)
