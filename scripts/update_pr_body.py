#!/usr/bin/env python3

"""
Script to generate a link to documentation preview in PRs.

This is expected to be run by the `reusable_update_pr_body.yml` GitHub workflow.

Requires the following packages:
  pip install PyGithub # NOLINT
"""
from __future__ import annotations

import argparse

from github import Github  # NOLINT

EMPTY_LINK = "<!-- This comment will be replaced by a link to the documentation preview -->"
LINK_START = "<!-- pr-link-docs:start -->"
LINK_END = "<!-- pr-link-docs:end -->"

LINK_TEMPLATE = """<!-- pr-link-docs:start -->
Docs preview: {{ docs-link }}
Examples preview: {{ examples-link }}
<!-- pr-link-docs:end -->"""


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    args = parser.parse_args()

    gh = Github(args.github_token)  # NOLINT
    repo = gh.get_repo(args.github_repository)
    pr = repo.get_pull(args.pr_number)

    latest_commit = pr.get_commits().reversed[0]

    print(f"Latest commit: {latest_commit.sha}")
    short_sha = latest_commit.sha[:7]

    body = pr.body

    # update preview links
    link = LINK_TEMPLATE.replace("{{ docs-link }}", f"https://rerun.io/preview/{short_sha}/docs")
    link = link.replace("{{ examples-link }}", f"https://rerun.io/preview/{short_sha}/examples")
    if EMPTY_LINK in body:
        print("Empty link found, updating it")
        body = body.replace(EMPTY_LINK, link)
    else:
        start = body.find(LINK_START)
        end = body.find(LINK_END)
        if start != -1 and end != -1:
            print("Existing link found, updating it")
            body = body[:start] + link + body[end + len(LINK_END) :]

    # update the pr number
    if "{{ pr-number }}" in body:
        body = body.replace("{{ pr-number }}", str(args.pr_number))

    if body != pr.body:
        pr.edit(body=body)


if __name__ == "__main__":
    main()
