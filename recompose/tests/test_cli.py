"""Tests for the CLI module."""

import click
from click.testing import CliRunner

import recompose
from recompose.cli import _build_command


def test_build_command_basic():
    @recompose.task
    def simple_task() -> recompose.Result[str]:
        """A simple task."""
        return recompose.Ok("done")

    info = simple_task._task_info
    cmd = _build_command(info)

    # Command name should be kebab-case
    assert cmd.name == "simple-task"
    assert cmd.help is not None
    assert "A simple task." in cmd.help


def test_build_command_with_args():
    @recompose.task
    def task_with_args(*, name: str, count: int = 1) -> recompose.Result[str]:
        """Task with arguments."""
        return recompose.Ok(f"{name} x {count}")

    info = task_with_args._task_info
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

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(help_test_task._task_info))

    result = runner.invoke(cli, ["--help"])
    assert result.exit_code == 0
    # Command name should be kebab-case in help
    assert "help-test-task" in result.output


def test_cli_task_help():
    @recompose.task
    def task_help_test(*, name: str, value: int = 42) -> recompose.Result[str]:
        """Task for testing help."""
        return recompose.Ok(f"{name}={value}")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(task_help_test._task_info))

    # Use kebab-case command name
    result = runner.invoke(cli, ["task-help-test", "--help"])
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

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(runnable_task._task_info))

    # Use kebab-case command name
    result = runner.invoke(cli, ["runnable-task", "--x=5", "--y=3"])
    assert result.exit_code == 0
    assert "succeeded" in result.output
    assert "8" in result.output


def test_cli_handles_failure():
    @recompose.task
    def failing_cli_task() -> recompose.Result[str]:
        """A task that fails."""
        raise ValueError("intentional error")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(failing_cli_task._task_info))

    # Use kebab-case command name
    result = runner.invoke(cli, ["failing-cli-task"])
    assert "failed" in result.output
    assert "ValueError: intentional error" in result.output


def test_cli_required_argument():
    @recompose.task
    def required_arg_task(*, required_param: str) -> recompose.Result[str]:
        """Task with required argument."""
        return recompose.Ok(required_param)

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(required_arg_task._task_info))

    # Should fail without required argument (use kebab-case command name)
    result = runner.invoke(cli, ["required-arg-task"])
    assert result.exit_code != 0
    assert "required" in result.output.lower()


def test_cli_optional_argument():
    @recompose.task
    def optional_arg_task(*, param: str = "default") -> recompose.Result[str]:
        """Task with optional argument."""
        return recompose.Ok(param)

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(optional_arg_task._task_info))

    # Should work without the optional argument (use kebab-case command name)
    result = runner.invoke(cli, ["optional-arg-task"])
    assert result.exit_code == 0
    assert "default" in result.output


def test_cli_bool_argument():
    @recompose.task
    def bool_task(*, flag: bool = False) -> recompose.Result[str]:
        """Task with bool flag."""
        return recompose.Ok(f"flag={flag}")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(bool_task._task_info))

    # Test with --flag (use kebab-case command name)
    result = runner.invoke(cli, ["bool-task", "--flag"])
    assert result.exit_code == 0
    assert "flag=True" in result.output

    # Test with --no-flag
    result = runner.invoke(cli, ["bool-task", "--no-flag"])
    assert result.exit_code == 0
    assert "flag=False" in result.output


def test_cli_float_argument():
    @recompose.task
    def float_task(*, value: float) -> recompose.Result[float]:
        """Task with float argument."""
        return recompose.Ok(value * 2)

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(float_task._task_info))

    # Use kebab-case command name
    result = runner.invoke(cli, ["float-task", "--value=3.14"])
    assert result.exit_code == 0
    assert "6.28" in result.output


def test_cli_kebab_case_arguments():
    """Test that parameter names with underscores become kebab-case CLI options."""

    @recompose.task
    def kebab_task(*, my_long_param: str, another_value: int = 42) -> recompose.Result[str]:
        """Task with underscore params."""
        return recompose.Ok(f"{my_long_param}={another_value}")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(kebab_task._task_info))

    # Help should show kebab-case options (use kebab-case command name)
    result = runner.invoke(cli, ["kebab-task", "--help"])
    assert result.exit_code == 0
    assert "--my-long-param" in result.output
    assert "--another-value" in result.output
    # Should NOT have underscore versions
    assert "--my_long_param" not in result.output
    assert "--another_value" not in result.output

    # Should work with kebab-case arguments
    result = runner.invoke(cli, ["kebab-task", "--my-long-param=hello", "--another-value=99"])
    assert result.exit_code == 0
    assert "hello=99" in result.output


def test_cli_kebab_case_bool_flags():
    """Test that bool flags with underscores become kebab-case CLI options."""

    @recompose.task
    def kebab_bool_task(*, full_tests: bool = False) -> recompose.Result[str]:
        """Task with underscore bool param."""
        return recompose.Ok(f"full_tests={full_tests}")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(kebab_bool_task._task_info))

    # Help should show kebab-case options (use kebab-case command name)
    result = runner.invoke(cli, ["kebab-bool-task", "--help"])
    assert result.exit_code == 0
    assert "--full-tests" in result.output
    assert "--no-full-tests" in result.output
    # Should NOT have underscore versions
    assert "--full_tests" not in result.output
    assert "--no-full_tests" not in result.output

    # Should work with kebab-case flags
    result = runner.invoke(cli, ["kebab-bool-task", "--full-tests"])
    assert result.exit_code == 0
    assert "full_tests=True" in result.output

    result = runner.invoke(cli, ["kebab-bool-task", "--no-full-tests"])
    assert result.exit_code == 0
    assert "full_tests=False" in result.output


def test_cli_kebab_case_command_names():
    """Test that command names with underscores become kebab-case."""

    @recompose.task
    def my_long_task_name() -> recompose.Result[str]:
        """Task with underscore name."""
        return recompose.Ok("done")

    runner = CliRunner()

    @click.group()
    def cli():
        pass

    cli.add_command(_build_command(my_long_task_name._task_info))

    # Help should show kebab-case command name
    result = runner.invoke(cli, ["--help"])
    assert result.exit_code == 0
    assert "my-long-task-name" in result.output
    assert "my_long_task_name" not in result.output

    # Command should work with kebab-case name
    result = runner.invoke(cli, ["my-long-task-name"])
    assert result.exit_code == 0
    assert "succeeded" in result.output
