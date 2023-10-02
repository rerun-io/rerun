#!/usr/bin/env python3

from __future__ import annotations

import argparse

from github import Github


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    args = parser.parse_args()

    gh = Github(args.github_token)
    repo = gh.get_repo(args.github_repository)
    pr = repo.get_pull(args.pr_number)
    if not pr.body:
        # body is empty
        print("Don't delete the PR description")
        exit(1)

    latest_commit = pr.get_commits().reversed[0]
    print(f"Latest commit: {latest_commit.sha}")

    body_lower = pr.body.lower()
    if "* [ ]" in body_lower or "- [ ]" in body_lower:
        print("PR contains unchecked checkboxes")
        exit(1)
    elif "* [x]" in body_lower or "- [x]" in body_lower:
        print("All clear")
        exit(0)
    else:
        print("Don't delete the PR description")
        exit(1)


if __name__ == "__main__":
    main()
