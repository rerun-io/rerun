#!/usr/bin/env python
from __future__ import annotations

import argparse
import asyncio
import os
import re

import aiohttp
from gitignore_parser import parse_gitignore

# ---

parser = argparse.ArgumentParser(description="Hunt down zombie TODOs.")
parser.add_argument("--token", dest="GITHUB_TOKEN", help="Github token to fetch issues", required=True)

args = parser.parse_args()

# --- Fetch issues from Github API ---

headers = {
    "Accept": "application/vnd.github+json",
    "Authorization": f"Bearer {args.GITHUB_TOKEN}",
    "X-GitHub-Api-Version": "2022-11-28",
}

issues: list[int] = []

repo_owner = "rerun-io"
repo_name = "rerun"
issue_state = "closed"
per_page = 100


async def fetch_issue_page(session: aiohttp.ClientSession, page: int) -> list[int]:
    url = f"https://api.github.com/repos/{repo_owner}/{repo_name}/issues?state={issue_state}&per_page={per_page}&page={page}"
    async with session.get(url, headers=headers) as response:
        if response.status != 200:
            print(f"Error: Failed to fetch issues from page {page}. Status code: {response.status}")
            return []
        data = await response.json()
        return [issue["number"] for issue in data]


async def fetch_total_number_of_issue_pages(session: aiohttp.ClientSession) -> int | None:
    url = f"https://api.github.com/repos/{repo_owner}/{repo_name}/issues?state={issue_state}&per_page={per_page}"
    async with session.get(url, headers=headers) as response:
        if response.status != 200:
            print(f"Error: Failed to fetch total pages. Status code: {response.status}")
            return None
        link_header = response.headers.get("Link")
        if link_header:
            match = re.search(r'page=(\d+)>; rel="last"', link_header)
            if match:
                return int(match.group(1))
        return None


async def fetch_issues() -> None:
    async with aiohttp.ClientSession() as session:
        total_pages = await fetch_total_number_of_issue_pages(session)
        if total_pages is None:
            print("Failed to determine the number of pages.")
            return

        tasks = [fetch_issue_page(session, page) for page in range(1, total_pages + 1)]
        issue_lists = await asyncio.gather(*tasks)
        issues.extend(issue for issue_list in issue_lists for issue in issue_list)


# --- Check files for zombie TODOs ---


internal_issue_number_pattern = re.compile(r"TODO\((?:#(\d+))(?:,\s*(?:#(\d+)))*\)")


# Returns true if the file is OK.
def check_file(path: str) -> bool:
    ok = True
    closed_issues = set(issues)
    with open(path, encoding="utf8") as f:
        for i, line in enumerate(f.readlines()):
            matches = internal_issue_number_pattern.search(line)
            if matches is not None:
                for match in matches.groups():
                    if match is not None and int(match) in closed_issues:
                        print(f"{path}:{i}: {line.strip()}")
                        ok &= False
    return ok


# ---


def main() -> None:
    asyncio.run(fetch_issues())

    script_dirpath = os.path.dirname(os.path.realpath(__file__))
    root_dirpath = os.path.abspath(f"{script_dirpath}/..")
    os.chdir(root_dirpath)

    extensions = ["c", "cpp", "fbs", "h", "hpp", "html", "js", "md", "py", "rs", "sh", "toml", "txt", "wgsl", "yml"]

    exclude_paths = {
        "./CODE_STYLE.md",
        "./scripts/lint.py",
        "./scripts/zombie_todos.py",
    }

    should_ignore = parse_gitignore(".gitignore")  # TODO(emilk): parse all .gitignore files, not just top-level

    ok = True
    for root, dirs, files in os.walk(".", topdown=True):
        dirs[:] = [d for d in dirs if not should_ignore(d)]

        for filename in files:
            extension = filename.split(".")[-1]
            if extension in extensions:
                filepath = os.path.join(root, filename)
                if should_ignore(filepath):
                    continue
                if filepath.replace("\\", "/") not in exclude_paths:
                    ok &= check_file(filepath)

    if not ok:
        raise ValueError("Clean your zombies!")


if __name__ == "__main__":
    main()
