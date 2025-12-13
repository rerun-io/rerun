"""Tests for the CLI module."""

from click.testing import CliRunner

import recompose
from recompose.cli import _build_command, main
from recompose.task import _task_registry, get_registry


def setup_function():
    """Clear the task registry before each test."""
    _task_registry.clear()


def test_build_command_basic():
    @recompose.task
    def simple_task() -> recompose.Result[str]:
        """A simple task."""
        return recompose.Ok("done")

    info = get_registry()[f"{simple_task.__module__}:simple_task"]
    cmd = _build_command(info)

    assert cmd.name == "simple_task"
    assert "A simple task." in cmd.help


def test_build_command_with_args():
    @recompose.task
    def task_with_args(*, name: str, count: int = 1) -> recompose.Result[str]:
        """Task with arguments."""
        return recompose.Ok(f"{name} x {count}")

    info = get_registry()[f"{task_with_args.__module__}:task_with_args"]
    cmd = _build_command(info)

    param_names = [p.name for p in cmd.params]
    assert "name" in param_names
    assert "count" in param_names


def test_cli_help():
    @recompose.task
    def help_test_task() -> recompose.Result[str]:
        """Help test task."""
        return recompose.Ok("done")

    runner = CliRunner()

    # We need to build the CLI manually for testing
    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    result = runner.invoke(cli, ["--help"])
    assert result.exit_code == 0
    assert "help_test_task" in result.output


def test_cli_task_help():
    @recompose.task
    def task_help_test(*, name: str, value: int = 42) -> recompose.Result[str]:
        """Task for testing help."""
        return recompose.Ok(f"{name}={value}")

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    result = runner.invoke(cli, ["task_help_test", "--help"])
    assert result.exit_code == 0
    assert "--name" in result.output
    assert "--value" in result.output
    assert "Task for testing help" in result.output


def test_cli_runs_task():
    @recompose.task
    def runnable_task(*, x: int, y: int) -> recompose.Result[int]:
        """Add two numbers."""
        return recompose.Ok(x + y)

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    result = runner.invoke(cli, ["runnable_task", "--x=5", "--y=3"])
    assert result.exit_code == 0
    assert "succeeded" in result.output
    assert "8" in result.output


def test_cli_handles_failure():
    @recompose.task
    def failing_cli_task() -> recompose.Result[str]:
        """A task that fails."""
        raise ValueError("intentional error")

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    result = runner.invoke(cli, ["failing_cli_task"])
    assert "failed" in result.output
    assert "ValueError: intentional error" in result.output


def test_cli_required_argument():
    @recompose.task
    def required_arg_task(*, required_param: str) -> recompose.Result[str]:
        """Task with required argument."""
        return recompose.Ok(required_param)

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    # Should fail without required argument
    result = runner.invoke(cli, ["required_arg_task"])
    assert result.exit_code != 0
    assert "required" in result.output.lower()


def test_cli_optional_argument():
    @recompose.task
    def optional_arg_task(*, param: str = "default") -> recompose.Result[str]:
        """Task with optional argument."""
        return recompose.Ok(param)

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    # Should work without the optional argument
    result = runner.invoke(cli, ["optional_arg_task"])
    assert result.exit_code == 0
    assert "default" in result.output


def test_cli_bool_argument():
    @recompose.task
    def bool_task(*, flag: bool = False) -> recompose.Result[str]:
        """Task with bool flag."""
        return recompose.Ok(f"flag={flag}")

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    # Test with --flag
    result = runner.invoke(cli, ["bool_task", "--flag"])
    assert result.exit_code == 0
    assert "flag=True" in result.output

    # Test with --no-flag
    result = runner.invoke(cli, ["bool_task", "--no-flag"])
    assert result.exit_code == 0
    assert "flag=False" in result.output


def test_cli_float_argument():
    @recompose.task
    def float_task(*, value: float) -> recompose.Result[float]:
        """Task with float argument."""
        return recompose.Ok(value * 2)

    runner = CliRunner()

    import click

    @click.group()
    def cli():
        pass

    for info in get_registry().values():
        cli.add_command(_build_command(info))

    result = runner.invoke(cli, ["float_task", "--value=3.14"])
    assert result.exit_code == 0
    assert "6.28" in result.output
