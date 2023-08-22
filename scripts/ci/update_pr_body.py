#!/usr/bin/env python3

"""
Script to update the PR description template.

This is expected to be run by the `reusable_update_pr_body.yml` GitHub workflow.

Requires the following packages:
  pip install Jinja2 PyGithub
"""
from __future__ import annotations

import argparse
import logging
import urllib.parse

from github import Github
from jinja2 import DebugUndefined, select_autoescape
from jinja2.sandbox import SandboxedEnvironment


def encode_uri_component(value: str) -> str:
    return urllib.parse.quote(value, safe="")


def main() -> None:
    logging.getLogger().setLevel(-1)
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    args = parser.parse_args()

    gh = Github(args.github_token)
    repo = gh.get_repo(args.github_repository)
    pr = repo.get_pull(args.pr_number)

    latest_commit = pr.get_commits().reversed[0]

    print(f"Latest commit: {latest_commit.sha}")

    env = SandboxedEnvironment(
        autoescape=select_autoescape(),
        undefined=DebugUndefined,
    )
    env.filters["encode_uri_component"] = encode_uri_component

    body = env.from_string(pr.body).render(
        pr={
            "number": args.pr_number,
            "branch": pr.head.ref,
            "commit": latest_commit,
        },
    )

    if body != pr.body:
        pr.edit(body=body)


if __name__ == "__main__":
    main()
