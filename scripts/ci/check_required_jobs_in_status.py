#!/usr/bin/env python3
"""
Generate the list of jobs for ci-status by analyzing workflow dependencies.

This script parses GitHub Actions workflow files and identifies all leaf jobs
(jobs that no other jobs depend on) to be included in the ci-status job's needs array.
"""

from __future__ import annotations

import sys
from pathlib import Path

import yaml

STATUS_JOB = "ci-status"


def parse_workflow(workflow_path: Path) -> dict[str, set[str]]:
    """
    Parse a workflow file and extract job dependencies.

    Returns:
        Dict mapping job_name -> set of jobs it depends on (from 'needs')

    """
    with open(workflow_path, encoding=None) as f:
        workflow = yaml.safe_load(f)

    if not workflow or "jobs" not in workflow:
        return {}

    job_dependencies: dict[str, set[str]] = {}

    for job_name, job_config in workflow["jobs"].items():
        if job_config is None:
            job_dependencies[job_name] = set()
            continue

        needs = job_config.get("needs", [])

        # Normalize needs to a list
        if isinstance(needs, str):
            needs = [needs]
        elif needs is None:
            needs = []

        job_dependencies[job_name] = set(needs)

    return job_dependencies


def find_leaf_jobs(job_dependencies: dict[str, set[str]]) -> list[str]:
    """
    Find all leaf jobs (jobs that no other jobs depend on).

    A leaf job is one that doesn't appear in any other job's 'needs' list.
    """
    all_jobs = set(job_dependencies.keys())
    jobs_depended_on = set()

    # Collect all jobs that are dependencies of other jobs
    for deps in job_dependencies.values():
        jobs_depended_on.update(deps)

    # Leaf jobs are those not depended on by anyone
    leaf_jobs = all_jobs - jobs_depended_on

    return sorted(leaf_jobs)


def main() -> None:
    """Parse workflow files and output job lists."""
    import argparse

    parser = argparse.ArgumentParser(description="Extract job dependencies from GitHub Actions workflows")
    parser.add_argument("workflow_files", nargs="+", type=Path, help="Workflow YAML files to parse")
    args = parser.parse_args()

    # Parse all workflow files
    all_dependencies = {}
    for workflow_path in args.workflow_files:
        if not workflow_path.exists():
            print(f"Warning: {workflow_path} does not exist", file=sys.stderr)
            continue

        deps = parse_workflow(workflow_path)
        all_dependencies.update(deps)

    if not all_dependencies:
        print("No jobs found in workflow files", file=sys.stderr)
        sys.exit(1)

    jobs = find_leaf_jobs(all_dependencies)
    jobs = [j for j in jobs if j != STATUS_JOB]

    if len(jobs) != 0:
        print(f"The following jobs are not included in the {STATUS_JOB}:\n{jobs}")
        sys.exit(1)

    print(f"All jobs are included in {STATUS_JOB}!")

if __name__ == "__main__":
    main()
