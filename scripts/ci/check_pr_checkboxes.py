#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys

from github import Github


def get_unchecked_checkboxes(s: str) -> list[str]:
    s = s.lower()
    lines: list[str] = []
    for line in s.splitlines():
        if "* [ ]" in line or "- [ ]" in line:
            lines.append(line)

    return lines


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

    if not pr.body:
        return

    checkboxes = get_unchecked_checkboxes(pr.body)

    if len(checkboxes) != 0:
        print("Unchecked checkboxes found:")
        for checkbox in checkboxes:
            print(f"  {checkbox}")
        sys.exit(1)


if __name__ == "__main__":
    main()
