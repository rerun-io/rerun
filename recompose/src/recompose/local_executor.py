"""Local execution of automations.

This module allows running automations locally, executing each job as a subprocess
with proper dependency ordering and output passing.

Usage:
    # From CLI
    ./run ci

    # Programmatically
    from recompose.local_executor import LocalExecutor
    executor = LocalExecutor(cli_command="./run")
    result = executor.execute(ci)
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

from rich.console import Console

from .jobs import ArtifactRef, InputParamRef, JobOutputRef, JobSpec

if TYPE_CHECKING:
    from .jobs import AutomationWrapper

console = Console()


@dataclass
class JobResult:
    """Result of executing a single job."""

    job_id: str
    success: bool
    elapsed_seconds: float
    outputs: dict[str, str] = field(default_factory=dict)
    error: str | None = None


@dataclass
class AutomationResult:
    """Result of executing an automation."""

    automation_name: str
    success: bool
    elapsed_seconds: float
    job_results: list[JobResult] = field(default_factory=list)

    @property
    def failed_jobs(self) -> list[JobResult]:
        """Get list of failed jobs."""
        return [j for j in self.job_results if not j.success]


def topological_sort(jobs: list[JobSpec]) -> list[JobSpec]:
    """
    Topologically sort jobs based on dependencies.

    Jobs are sorted so that dependencies come before dependents.
    Raises ValueError if there's a cycle.
    """
    # Build adjacency list and in-degree count
    job_map = {j.job_id: j for j in jobs}
    in_degree: dict[str, int] = {j.job_id: 0 for j in jobs}
    dependents: dict[str, list[str]] = {j.job_id: [] for j in jobs}

    for job in jobs:
        for dep in job.get_all_dependencies():
            if dep.job_id in job_map:  # Only count deps within this automation
                in_degree[job.job_id] += 1
                dependents[dep.job_id].append(job.job_id)

    # Kahn's algorithm
    queue = [jid for jid, deg in in_degree.items() if deg == 0]
    result: list[JobSpec] = []

    while queue:
        job_id = queue.pop(0)
        result.append(job_map[job_id])
        for dependent_id in dependents[job_id]:
            in_degree[dependent_id] -= 1
            if in_degree[dependent_id] == 0:
                queue.append(dependent_id)

    if len(result) != len(jobs):
        # Find the cycle for error message
        remaining = [j.job_id for j in jobs if j not in result]
        raise ValueError(f"Dependency cycle detected involving jobs: {remaining}")

    return result


def _group_jobs_by_level(jobs: list[JobSpec]) -> list[list[JobSpec]]:
    """
    Group jobs into levels for parallel execution.

    Jobs at the same level have no dependencies on each other and can run in parallel.
    Level N+1 jobs depend on level N jobs completing.

    Returns a list of levels, where each level is a list of jobs that can run in parallel.
    Raises ValueError if there's a cycle.
    """
    if not jobs:
        return []

    job_map = {j.job_id: j for j in jobs}
    in_degree: dict[str, int] = {j.job_id: 0 for j in jobs}
    dependents: dict[str, list[str]] = {j.job_id: [] for j in jobs}

    for job in jobs:
        for dep in job.get_all_dependencies():
            if dep.job_id in job_map:
                in_degree[job.job_id] += 1
                dependents[dep.job_id].append(job.job_id)

    # Group by levels using modified Kahn's algorithm
    levels: list[list[JobSpec]] = []
    remaining = set(job_map.keys())

    while remaining:
        # Find all jobs with no remaining dependencies
        level = [job_map[jid] for jid in remaining if in_degree[jid] == 0]

        if not level:
            # Cycle detected
            raise ValueError(f"Dependency cycle detected involving jobs: {list(remaining)}")

        levels.append(level)

        # Remove this level from consideration
        for job in level:
            remaining.remove(job.job_id)
            for dependent_id in dependents[job.job_id]:
                in_degree[dependent_id] -= 1

    return levels


def _resolve_input_value(
    value: Any,
    job_outputs: dict[str, dict[str, str]],
    input_params: dict[str, Any],
) -> Any:
    """
    Resolve an input value, replacing refs with actual values.

    Args:
        value: The input value (may be a ref type or literal)
        job_outputs: Map of job_id -> {output_name -> value}
        input_params: Map of param_name -> value

    Returns:
        The resolved value

    """
    if isinstance(value, JobOutputRef):
        job_outputs_for_job = job_outputs.get(value.job_id, {})
        if value.output_name not in job_outputs_for_job:
            raise ValueError(
                f"Output '{value.output_name}' not found in job '{value.job_id}'. "
                f"Available outputs: {list(job_outputs_for_job.keys())}"
            )
        return job_outputs_for_job[value.output_name]

    elif isinstance(value, ArtifactRef):
        # For local execution, artifacts are just file paths
        # The producing job should have set an output with the artifact path
        # For now, we'll just return a placeholder - artifact handling is deferred
        return f"<artifact:{value.job_id}/{value.artifact_name}>"

    elif isinstance(value, InputParamRef):
        if value.param_name not in input_params:
            raise ValueError(
                f"Input parameter '{value.param_name}' not provided. Available params: {list(input_params.keys())}"
            )
        return input_params[value.param_name]

    else:
        # Literal value
        return value


def _build_cli_args(
    inputs: dict[str, Any],
    job_outputs: dict[str, dict[str, str]],
    input_params: dict[str, Any],
) -> list[str]:
    """
    Build CLI arguments from job inputs.

    Args:
        inputs: The job's input dict
        job_outputs: Map of job_id -> {output_name -> value}
        input_params: Map of param_name -> value

    Returns:
        List of CLI arguments like ["--arg1=value1", "--arg2=value2"]

    """
    args: list[str] = []
    for name, value in inputs.items():
        resolved = _resolve_input_value(value, job_outputs, input_params)
        # Convert to CLI argument
        # Convert underscores to hyphens for CLI style
        cli_name = name.replace("_", "-")
        if isinstance(resolved, bool):
            if resolved:
                args.append(f"--{cli_name}")
            else:
                args.append(f"--no-{cli_name}")
        else:
            args.append(f"--{cli_name}={resolved}")
    return args


def _parse_github_output(output_file: Path) -> dict[str, str]:
    """
    Parse a GITHUB_OUTPUT file and return the key-value pairs.

    Handles both simple `key=value` format and multiline delimiter format.
    """
    outputs: dict[str, str] = {}
    if not output_file.exists():
        return outputs

    content = output_file.read_text()
    lines = content.split("\n")

    i = 0
    while i < len(lines):
        line = lines[i]
        if "=" in line and "<<" not in line:
            # Simple key=value
            key, value = line.split("=", 1)
            outputs[key] = value
            i += 1
        elif "<<" in line:
            # Multiline format: key<<DELIMITER
            key, delimiter = line.split("<<", 1)
            delimiter = delimiter.strip()
            i += 1
            value_lines = []
            while i < len(lines) and lines[i].strip() != delimiter:
                value_lines.append(lines[i])
                i += 1
            outputs[key] = "\n".join(value_lines)
            i += 1  # Skip delimiter line
        else:
            i += 1

    return outputs


class LocalExecutor:
    """
    Executes automations locally by running jobs as subprocesses.

    Each job is run via the CLI command (e.g., `./run task-name --arg=value`).
    Job outputs are captured via temporary GITHUB_OUTPUT files and passed
    to dependent jobs.

    Args:
        cli_command: The CLI entry point (e.g., "./run")
        working_directory: Working directory for subprocess execution
        dry_run: If True, print what would be run without executing
        verbose: If True, show verbose output

    Example:
        executor = LocalExecutor(cli_command="./run")
        result = executor.execute(ci, verbose=True)
        if not result.success:
            print(f"Failed jobs: {result.failed_jobs}")

    """

    def __init__(
        self,
        *,
        cli_command: str = "./run",
        working_directory: Path | str | None = None,
        dry_run: bool = False,
        verbose: bool = False,
    ):
        self.cli_command = cli_command
        self.working_directory = Path(working_directory) if working_directory else None
        self.dry_run = dry_run
        self.verbose = verbose

    def execute(
        self,
        automation: AutomationWrapper | Callable[..., list[JobSpec]],
        **input_params: Any,
    ) -> AutomationResult:
        """
        Execute an automation locally.

        Args:
            automation: The automation to execute (AutomationWrapper)
            **input_params: Values for InputParam parameters

        Returns:
            AutomationResult with job results and overall status

        """
        start_time = time.perf_counter()

        # Get automation name
        if hasattr(automation, "info"):
            automation_name = automation.info.name
        elif hasattr(automation, "__name__"):
            automation_name = automation.__name__
        else:
            automation_name = "automation"

        # Build the job graph
        console.print(f"\n[bold blue]▶[/bold blue] Running automation: [bold]{automation_name}[/bold]")

        # Execute automation to get jobs
        jobs = automation(**input_params)

        if not jobs:
            console.print("[yellow]No jobs to execute[/yellow]")
            return AutomationResult(
                automation_name=automation_name,
                success=True,
                elapsed_seconds=time.perf_counter() - start_time,
            )

        # Group jobs into levels for parallel execution
        try:
            levels = _group_jobs_by_level(jobs)
        except ValueError as e:
            console.print(f"[red]Error:[/red] {e}")
            return AutomationResult(
                automation_name=automation_name,
                success=False,
                elapsed_seconds=time.perf_counter() - start_time,
            )

        if self.verbose:
            level_strs = []
            for level in levels:
                if len(level) == 1:
                    level_strs.append(level[0].job_id)
                else:
                    level_strs.append(f"[{', '.join(j.job_id for j in level)}]")
            console.print(f"[dim]Execution order: {' → '.join(level_strs)}[/dim]")

        # Execute jobs level by level (parallel within each level)
        job_outputs: dict[str, dict[str, str]] = {}
        job_results: list[JobResult] = []
        failed_jobs: set[str] = set()

        for level in levels:
            # Filter out jobs whose dependencies failed
            runnable = [j for j in level if not any(dep.job_id in failed_jobs for dep in j.get_all_dependencies())]
            skipped = [j for j in level if j not in runnable]

            # Mark skipped jobs
            for job_spec in skipped:
                job_results.append(
                    JobResult(
                        job_id=job_spec.job_id,
                        success=False,
                        elapsed_seconds=0,
                        error="Skipped due to failed dependency",
                    )
                )
                failed_jobs.add(job_spec.job_id)

            if not runnable:
                continue

            # Run jobs in parallel (or sequentially if only one)
            if len(runnable) == 1:
                # Single job - run directly with live output
                result = self._execute_job(runnable[0], job_outputs, input_params, buffer_output=False)
                job_results.append(result)
                if result.success:
                    job_outputs[runnable[0].job_id] = result.outputs
                else:
                    failed_jobs.add(runnable[0].job_id)
            else:
                # Multiple jobs - run in parallel with buffered output
                from concurrent.futures import ThreadPoolExecutor

                # Show that we're running jobs in parallel
                job_names = ", ".join(j.job_id for j in runnable)
                console.print(f"\n  [bold cyan]⊕[/bold cyan] Running in parallel: [bold]{job_names}[/bold]")

                results_map: dict[str, JobResult] = {}
                with ThreadPoolExecutor(max_workers=len(runnable)) as executor:
                    futures = {
                        executor.submit(
                            self._execute_job, job_spec, job_outputs, input_params, buffer_output=True
                        ): job_spec
                        for job_spec in runnable
                    }
                    for future in futures:
                        job_spec = futures[future]
                        result = future.result()
                        results_map[job_spec.job_id] = result

                # Print results in consistent order
                for job_spec in runnable:
                    result = results_map[job_spec.job_id]
                    job_results.append(result)
                    self._print_job_result(job_spec, result)
                    if result.success:
                        job_outputs[job_spec.job_id] = result.outputs
                    else:
                        failed_jobs.add(job_spec.job_id)

        # Summary
        elapsed = time.perf_counter() - start_time
        success = all(r.success for r in job_results)

        console.print()
        if success:
            console.print(
                f"[bold green]✓[/bold green] Automation [bold]{automation_name}[/bold] "
                f"completed in {elapsed:.2f}s ({len(job_results)} jobs)"
            )
        else:
            failed = [r for r in job_results if not r.success]
            console.print(
                f"[bold red]✗[/bold red] Automation [bold]{automation_name}[/bold] "
                f"failed in {elapsed:.2f}s ({len(job_results) - len(failed)}/{len(job_results)} jobs passed)"
            )

        return AutomationResult(
            automation_name=automation_name,
            success=success,
            elapsed_seconds=elapsed,
            job_results=job_results,
        )

    def _execute_job(
        self,
        job_spec: JobSpec,
        job_outputs: dict[str, dict[str, str]],
        input_params: dict[str, Any],
        buffer_output: bool = False,
    ) -> JobResult:
        """
        Execute a single job as a subprocess.

        Args:
            job_spec: The job to execute
            job_outputs: Outputs from previously completed jobs
            input_params: Input parameter values
            buffer_output: If True, don't print output (for parallel execution)

        Returns:
            JobResult with success status, outputs, and captured output lines

        """
        start_time = time.perf_counter()
        job_id = job_spec.job_id
        task_name = job_spec.task_info.name

        # Convert task name to CLI format (kebab-case)
        cli_task_name = task_name.replace("_", "-")

        # Build CLI command
        cli_args = _build_cli_args(job_spec.inputs, job_outputs, input_params)
        cmd = [self.cli_command, cli_task_name] + cli_args

        # Print job header (unless buffering)
        if not buffer_output:
            console.print(f"\n  [bold cyan]→[/bold cyan] [bold]{job_id}[/bold]", end="")
            if self.verbose:
                console.print(f" [dim]({' '.join(cmd)})[/dim]")
            else:
                console.print()

        if self.dry_run:
            if not buffer_output:
                console.print(f"    [dim]Would run: {' '.join(cmd)}[/dim]")
            return JobResult(
                job_id=job_id,
                success=True,
                elapsed_seconds=0,
            )

        # Create temp file for GITHUB_OUTPUT
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            output_file = Path(f.name)

        try:
            # Set up environment
            env = os.environ.copy()
            env["GITHUB_OUTPUT"] = str(output_file)
            env["RECOMPOSE_SUBPROCESS"] = "1"  # Suppress task headers in subprocess

            # Run the subprocess
            process = subprocess.Popen(
                cmd,
                cwd=None,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )

            # Collect output
            prefix = "    │ "
            output_lines: list[str] = []
            assert process.stdout is not None
            for line in process.stdout:
                line = line.rstrip("\n")
                output_lines.append(line)
                # Stream output if not buffering and verbose
                if not buffer_output and self.verbose:
                    console.print(f"[dim]{prefix}[/dim]{line}")

            process.wait()
            elapsed = time.perf_counter() - start_time

            # Parse outputs
            outputs = _parse_github_output(output_file)

            # Create result with captured output
            result = JobResult(
                job_id=job_id,
                success=process.returncode == 0,
                elapsed_seconds=elapsed,
                outputs=outputs if process.returncode == 0 else {},
                error="\n".join(output_lines[-10:]) if process.returncode != 0 and output_lines else None,
            )
            # Store output lines for later printing
            result._output_lines = output_lines  # type: ignore[attr-defined]

            # Print result (unless buffering)
            if not buffer_output:
                self._print_job_result(job_spec, result, show_header=False)

            return result

        finally:
            # Clean up temp file
            if output_file.exists():
                output_file.unlink()

    def _print_job_result(self, job_spec: JobSpec, result: JobResult, show_header: bool = True) -> None:
        """Print the result of a job execution."""
        prefix = "    │ "
        output_lines: list[str] = getattr(result, "_output_lines", [])

        # Print header
        if show_header:
            console.print(f"\n  [bold cyan]→[/bold cyan] [bold]{result.job_id}[/bold]")

        # Print verbose output if enabled
        if self.verbose and output_lines:
            for line in output_lines:
                console.print(f"[dim]{prefix}[/dim]{line}")

        if result.success:
            console.print(f"    [green]✓[/green] completed in {result.elapsed_seconds:.2f}s")
            if result.outputs and self.verbose:
                for k, v in result.outputs.items():
                    console.print(f"    [dim]output: {k}={v}[/dim]")
        else:
            console.print(f"    [red]✗[/red] failed in {result.elapsed_seconds:.2f}s")
            if not self.verbose and output_lines:
                # Show last few lines of output as error context
                error_lines = output_lines[-10:]
                for line in error_lines:
                    console.print(f"[dim]{prefix}[/dim]{line}")


def execute_automation(
    automation: AutomationWrapper | Callable[..., list[JobSpec]],
    *,
    cli_command: str = "./run",
    working_directory: Path | str | None = None,
    dry_run: bool = False,
    verbose: bool = False,
    **input_params: Any,
) -> AutomationResult:
    """
    Execute an automation locally.

    This is a convenience function that creates a LocalExecutor and runs
    the automation.

    Args:
        automation: The automation to execute
        cli_command: CLI entry point (default: "./run")
        working_directory: Working directory for execution
        dry_run: If True, show what would run without executing
        verbose: If True, show verbose output
        **input_params: Values for InputParam parameters

    Returns:
        AutomationResult with job results and overall status

    Example:
        result = execute_automation(ci, verbose=True)
        if not result.success:
            sys.exit(1)

    """
    executor = LocalExecutor(
        cli_command=cli_command,
        working_directory=working_directory,
        dry_run=dry_run,
        verbose=verbose,
    )
    return executor.execute(automation, **input_params)
