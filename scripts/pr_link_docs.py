#!/usr/bin/env python3

"""
Script to generate a link to documentation preview in PRs.

This is expected to be run by the `reusable_pr_link_docs.yml` GitHub workflow.

Requires the following packages:
  pip install PyGithub # NOLINT
"""

import argparse

from github import Github  # NOLINT

EMPTY_LINK = "<!-- This comment will be replaced by a link to the documentation preview -->"
LINK_START = "<!-- pr-link-docs:start -->"
LINK_END = "<!-- pr-link-docs:end -->"

LINK_TEMPLATE = "<!-- pr-link-docs:start -->\nDocs preview: {{ link }}\n<!-- pr-link-docs:end -->"


def main() -> None:
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

    link = LINK_TEMPLATE.replace("{{ link }}", f"https://rerun.io/preview/{latest_commit.sha[:7]}/docs")
    if EMPTY_LINK in pr.body:
        print("Empty link found, updating it")
        new_body = pr.body.replace(EMPTY_LINK, link)
        pr.edit(body=new_body)
    else:
        start = pr.body.find(LINK_START)
        end = pr.body.find(LINK_END)
        if start != -1 and end != -1:
            print("Existing link found, updating it")
            new_body = pr.body[:start] + link + pr.body[end + len(LINK_END) :]
            pr.edit(body=new_body)


if __name__ == "__main__":
    main()
