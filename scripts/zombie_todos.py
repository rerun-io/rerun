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
parser.add_argument("--markdown", action="store_true", help="Format output as markdown checklist")

args = parser.parse_args()

# --- Fetch issues from Github API ---

headers = {
    "Accept": "application/vnd.github+json",
    "Authorization": f"Bearer {args.GITHUB_TOKEN}",
    "X-GitHub-Api-Version": "2022-11-28",
}

issues: list[int] = []
authors: dict[int, str] = {}

repo_owner = "rerun-io"
repo_name = "rerun"
issue_state = "closed"
per_page = 200


async def fetch_issues() -> None:
    async with aiohttp.ClientSession() as session:
        tasks: list[asyncio.Task[list[int]]] = []
        url = f"https://api.github.com/repos/{repo_owner}/{repo_name}/issues?state={issue_state}&per_page={per_page}"
        async with session.get(url, headers=headers) as response:
            if response.status != 200:
                print(f"Error: Failed to fetch first issue page. Status code: {response.status}")
                return None

            data = await response.json()
            issues.extend([issue["number"] for issue in data])
            for issue in data:
                authors[issue["number"]] = issue["user"]["login"]
            links = response.links

            while "next" in links:
                async with session.get(links["next"]["url"], headers=headers) as response:
                    if response.status != 200:
                        print(f"Error: Failed to fetch next issue page. Status code: {response.status}")
                        return None
                    data = await response.json()
                    issues.extend([issue["number"] for issue in data])
                    for issue in data:
                        authors[issue["number"]] = issue["user"]["login"]
                    links = response.links
                    print("fetched", len(issues), "issues")

        print("done fetching issues")
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
                        issue_num = int(match)
                        author = authors.get(issue_num, "unknown")
                        if args.markdown:
                            # Convert path to relative path for clean display
                            display_path = path.lstrip("./")
                            github_url = f"https://github.com/{repo_owner}/{repo_name}/blob/main/{display_path}#L{i+1}"
                            print(f"* [ ] `{line.strip()}`")
                            print(f"   * #{issue_num} (Issue author: @{author})")
                            print(f"   * [`{display_path}#L{i}`]({github_url})")
                        else:
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
