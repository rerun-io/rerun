"""Local execution of automations.

This module allows running automations locally, executing each job as a subprocess
with proper dependency ordering and output passing.

The output model is recursive:
1. Parent prints child's header
2. Parent executes child, capturing ALL output
3. Parent prefixes ALL captured output with continuation prefix
4. Parent prints status with SAME prefix
5. Move to next child
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .jobs import ArtifactRef, InputParamRef, JobOutputRef, JobSpec
from .output import get_output_manager

if TYPE_CHECKING:
    from .jobs import AutomationWrapper


@dataclass
class JobResult:
    """Result of executing a single job."""

    job_id: str
    success: bool
    elapsed_seconds: float
    outputs: dict[str, str] = field(default_factory=dict)
    output_text: str = ""  # Captured output from the job
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
    job_map = {j.job_id: j for j in jobs}
    in_degree: dict[str, int] = {j.job_id: 0 for j in jobs}
    dependents: dict[str, list[str]] = {j.job_id: [] for j in jobs}

    for job in jobs:
        for dep in job.get_all_dependencies():
            if dep.job_id in job_map:
                in_degree[job.job_id] += 1
                dependents[dep.job_id].append(job.job_id)

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
        remaining = [j.job_id for j in jobs if j not in result]
        raise ValueError(f"Dependency cycle detected involving jobs: {remaining}")

    return result


def _group_jobs_by_level(jobs: list[JobSpec]) -> list[list[JobSpec]]:
    """Group jobs into levels for parallel execution."""
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

    levels: list[list[JobSpec]] = []
    remaining = set(job_map.keys())

    while remaining:
        level = [job_map[jid] for jid in remaining if in_degree[jid] == 0]
        if not level:
            raise ValueError(f"Dependency cycle detected involving jobs: {list(remaining)}")
        levels.append(level)
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
    """Resolve an input value, replacing refs with actual values."""
    if isinstance(value, JobOutputRef):
        job_outputs_for_job = job_outputs.get(value.job_id, {})
        if value.output_name not in job_outputs_for_job:
            raise ValueError(
                f"Output '{value.output_name}' not found in job '{value.job_id}'. "
                f"Available outputs: {list(job_outputs_for_job.keys())}"
            )
        return job_outputs_for_job[value.output_name]
    elif isinstance(value, ArtifactRef):
        return f"<artifact:{value.job_id}/{value.artifact_name}>"
    elif isinstance(value, InputParamRef):
        if value.param_name not in input_params:
            raise ValueError(
                f"Input parameter '{value.param_name}' not provided. Available params: {list(input_params.keys())}"
            )
        return input_params[value.param_name]
    else:
        return value


def _build_cli_args(
    inputs: dict[str, Any],
    job_outputs: dict[str, dict[str, str]],
    input_params: dict[str, Any],
) -> list[str]:
    """Build CLI arguments from job inputs."""
    args: list[str] = []
    for name, value in inputs.items():
        resolved = _resolve_input_value(value, job_outputs, input_params)
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
    """Parse a GITHUB_OUTPUT file and return the key-value pairs."""
    outputs: dict[str, str] = {}
    if not output_file.exists():
        return outputs

    content = output_file.read_text()
    lines = content.split("\n")

    i = 0
    while i < len(lines):
        line = lines[i]
        if "=" in line and "<<" not in line:
            key, value = line.split("=", 1)
            outputs[key] = value
            i += 1
        elif "<<" in line:
            key, delimiter = line.split("<<", 1)
            delimiter = delimiter.strip()
            i += 1
            value_lines = []
            while i < len(lines) and lines[i].strip() != delimiter:
                value_lines.append(lines[i])
                i += 1
            outputs[key] = "\n".join(value_lines)
            i += 1
        else:
            i += 1

    return outputs


class LocalExecutor:
    """
    Executes automations locally by running jobs as subprocesses.

    Uses a recursive output model where each level captures child output
    and prefixes it uniformly.
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
        """Execute an automation locally."""
        start_time = time.perf_counter()
        output_mgr = get_output_manager()

        # Get automation name
        if hasattr(automation, "info"):
            automation_name = automation.info.name
        elif hasattr(automation, "__name__"):
            automation_name = automation.__name__
        else:
            automation_name = "automation"

        # Print automation header
        output_mgr.print_automation_header(automation_name)

        # Execute automation to get jobs
        jobs = automation(**input_params)

        if not jobs:
            output_mgr.print("No jobs to execute", style="yellow")
            return AutomationResult(
                automation_name=automation_name,
                success=True,
                elapsed_seconds=time.perf_counter() - start_time,
            )

        # Group jobs into levels for parallel execution
        try:
            levels = _group_jobs_by_level(jobs)
        except ValueError as e:
            output_mgr.print_error(str(e))
            return AutomationResult(
                automation_name=automation_name,
                success=False,
                elapsed_seconds=time.perf_counter() - start_time,
            )

        # Execute jobs level by level
        job_outputs: dict[str, dict[str, str]] = {}
        job_results: list[JobResult] = []
        failed_jobs: set[str] = set()

        for level_idx, level in enumerate(levels):
            is_last_level = level_idx == len(levels) - 1

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

            # Execute jobs
            if len(runnable) == 1:
                # Single job - execute directly
                job_spec = runnable[0]
                is_last = is_last_level
                result = self._execute_and_print_job(job_spec, job_outputs, input_params, is_last=is_last)
                job_results.append(result)
                if result.success:
                    job_outputs[job_spec.job_id] = result.outputs
                else:
                    failed_jobs.add(job_spec.job_id)
            else:
                # Multiple jobs - run in parallel, then print sequentially
                # Parallel group is a recursive execution level
                from concurrent.futures import ThreadPoolExecutor
                from io import StringIO

                from .output import PARALLEL_PREFIX

                # 1. Print parallel header
                output_mgr.print_parallel_header()

                # 2. Execute all jobs in parallel (capturing output)
                results_map: dict[str, JobResult] = {}
                with ThreadPoolExecutor(max_workers=len(runnable)) as executor:
                    futures = {
                        executor.submit(self._execute_job, job_spec, job_outputs, input_params): job_spec
                        for job_spec in runnable
                    }
                    for future in futures:
                        job_spec = futures[future]
                        result = future.result()
                        results_map[job_spec.job_id] = result

                # 3. Capture all job result printing into a buffer
                buffer = StringIO()
                old_stdout = sys.stdout
                sys.stdout = buffer
                try:
                    for idx, job_spec in enumerate(runnable):
                        result = results_map[job_spec.job_id]
                        is_last = idx == len(runnable) - 1
                        self._print_job_result(result, is_last=is_last)
                        job_results.append(result)
                        if result.success:
                            job_outputs[job_spec.job_id] = result.outputs
                        else:
                            failed_jobs.add(job_spec.job_id)
                finally:
                    sys.stdout = old_stdout

                # 4. Prefix all captured output and print
                captured = buffer.getvalue()
                if captured:
                    from .output import prefix_lines

                    print(prefix_lines(captured.rstrip("\n"), PARALLEL_PREFIX), flush=True)

        # Summary
        elapsed = time.perf_counter() - start_time
        success = all(r.success for r in job_results)

        output_mgr.print_automation_status(automation_name, success, elapsed, len(job_results))

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
    ) -> JobResult:
        """Execute a single job, capturing all output."""
        start_time = time.perf_counter()
        job_id = job_spec.job_id
        task_name = job_spec.task_info.name
        cli_task_name = task_name.replace("_", "-")

        # Build CLI command
        cli_args = _build_cli_args(job_spec.inputs, job_outputs, input_params)
        cmd = [self.cli_command, cli_task_name] + cli_args

        if self.dry_run:
            return JobResult(
                job_id=job_id,
                success=True,
                elapsed_seconds=0,
                output_text=f"Would run: {' '.join(cmd)}\n",
            )

        # Create temp file for GITHUB_OUTPUT
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            output_file = Path(f.name)

        try:
            env = os.environ.copy()
            env["GITHUB_OUTPUT"] = str(output_file)
            env["RECOMPOSE_SUBPROCESS"] = "1"

            # Propagate color settings to subprocess:
            # - NO_COLOR takes precedence (already in env if set)
            # - FORCE_COLOR is propagated if set
            # - Otherwise, set FORCE_COLOR if terminal supports color
            if "NO_COLOR" not in env:
                if "FORCE_COLOR" not in env and get_output_manager().colors_enabled:
                    env["FORCE_COLOR"] = "1"

            # Run subprocess and capture output
            process = subprocess.Popen(
                cmd,
                cwd=None,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )

            output_lines: list[str] = []
            assert process.stdout is not None
            for line in process.stdout:
                output_lines.append(line.rstrip("\n"))

            process.wait()
            elapsed = time.perf_counter() - start_time

            outputs = _parse_github_output(output_file)

            return JobResult(
                job_id=job_id,
                success=process.returncode == 0,
                elapsed_seconds=elapsed,
                outputs=outputs if process.returncode == 0 else {},
                output_text="\n".join(output_lines),
                error="\n".join(output_lines[-10:]) if process.returncode != 0 and output_lines else None,
            )

        finally:
            if output_file.exists():
                output_file.unlink()

    def _execute_and_print_job(
        self,
        job_spec: JobSpec,
        job_outputs: dict[str, dict[str, str]],
        input_params: dict[str, Any],
        is_last: bool = False,
    ) -> JobResult:
        """Execute a job and print its output with proper prefixing."""
        # Execute the job (captures output)
        result = self._execute_job(job_spec, job_outputs, input_params)

        # Print using recursive model
        self._print_job_result(result, is_last=is_last)

        return result

    def _print_job_result(self, result: JobResult, is_last: bool = False) -> None:
        """Print job result using recursive output model."""
        output_mgr = get_output_manager()

        # 1. Print header
        output_mgr.print_header(result.job_id, is_last=is_last)

        # 2. Get continuation prefix based on is_last
        prefix = output_mgr.get_continuation_prefix(is_last)

        # 3. Print captured output with prefix (if verbose or failed)
        if self.verbose and result.output_text:
            output_mgr.print_prefixed(result.output_text, prefix)
        elif not result.success and result.output_text:
            # Show last few lines on failure
            lines = result.output_text.split("\n")[-10:]
            output_mgr.print_prefixed("\n".join(lines), prefix)

        # 4. Print status with SAME prefix (styled)
        output_mgr.print_status(result.success, result.elapsed_seconds, prefix=prefix)

        # 5. Print outputs if verbose (after status, dimmed)
        if result.success and result.outputs and self.verbose:
            for k, v in result.outputs.items():
                output_mgr.print(prefix, style="bold cyan", end="")
                output_mgr.print(f"output: {k}={v}", style="dim")


def execute_automation(
    automation: AutomationWrapper | Callable[..., list[JobSpec]],
    *,
    cli_command: str = "./run",
    working_directory: Path | str | None = None,
    dry_run: bool = False,
    verbose: bool = False,
    **input_params: Any,
) -> AutomationResult:
    """Execute an automation locally."""
    executor = LocalExecutor(
        cli_command=cli_command,
        working_directory=working_directory,
        dry_run=dry_run,
        verbose=verbose,
    )
    return executor.execute(automation, **input_params)
