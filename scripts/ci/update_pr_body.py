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

DOCS_PREVIEW_MARKER = "<!--DOCS-PREVIEW-->"
DOCS_PREVIEW_BARE_LINK = "- [Docs preview](https://rerun.io/preview/{{ pr.commit }}/docs) <!--DOCS-PREVIEW-->"
EXAMPLES_PREVIEW_MARKER = "<!--EXAMPLES-PREVIEW-->"
EXAMPLES_PREVIEW_BARE_LINK = (
    "- [Examples preview](https://rerun.io/preview/{{ pr.commit }}/examples) <!--EXAMPLES-PREVIEW-->"
)


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

    new_body = pr.body

    docs_preview_link_end = new_body.find(DOCS_PREVIEW_MARKER)
    if docs_preview_link_end != -1:
        docs_preview_link_end += len(DOCS_PREVIEW_MARKER)
        docs_preview_link_start = new_body.rfind("\n", 0, docs_preview_link_end) + 1
        new_body = new_body[:docs_preview_link_start] + DOCS_PREVIEW_BARE_LINK + new_body[docs_preview_link_end:]

    examples_preview_link_end = new_body.find(EXAMPLES_PREVIEW_MARKER)
    if examples_preview_link_end != -1:
        len(EXAMPLES_PREVIEW_MARKER)
        examples_preview_link_start = new_body.rfind("\n", 0, examples_preview_link_end) + 1
        new_body = (
            new_body[:examples_preview_link_start] + EXAMPLES_PREVIEW_BARE_LINK + new_body[examples_preview_link_end:]
        )

    new_body = env.from_string(new_body).render(
        pr={
            "number": args.pr_number,
            "branch": pr.head.ref,
            "commit": latest_commit.sha,
        },
    )

    if new_body != pr.body:
        print("updated pr body")
        pr.edit(body=new_body)


if __name__ == "__main__":
    main()
