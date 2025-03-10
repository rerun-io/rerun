#!/usr/bin/env python3

"""
Script to auto-approve workflow runs if certain criteria are met.

Checks for a `@rerun-bot approve` comment made by an official Rerun team member,
and approves any workflow runs with pending approval.

This is expected to be run by the `auto_approve.yml` GitHub workflow.
"""

from __future__ import annotations

import argparse

import requests
from github import Github
from github.WorkflowRun import WorkflowRun

APPROVAL = "@rerun-bot approve"


def approve(github_token: str, workflow_run: WorkflowRun) -> None:
    print(f"approving {workflow_run.id}")
    requests.post(
        f"https://api.github.com/repos/rerun-io/rerun/actions/runs/{workflow_run.id}/approve",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {github_token}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    ).raise_for_status()


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    args = parser.parse_args()

    gh = Github(args.github_token)
    repo = gh.get_repo(args.github_repository)
    pr = repo.get_pull(args.pr_number)

    for comment in pr.get_issue_comments():
        if APPROVAL not in comment.body:
            continue

        can_user_approve_workflows = (
            repo.owner.login == comment.user.login
            or repo.organization.has_in_members(comment.user)
            or repo.has_in_collaborators(comment.user)
        )
        if not can_user_approve_workflows:
            continue

        print(f"found valid approval by {comment.user.login}")
        for workflow_run in repo.get_workflow_runs(branch=repo.get_branch(pr.head.ref)):
            if workflow_run.status == "action_required" or workflow_run.conclusion == "action_required":
                approve(args.github_token, workflow_run)

        # We only need one approval
        return


if __name__ == "__main__":
    main()
