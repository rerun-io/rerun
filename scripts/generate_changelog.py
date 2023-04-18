#!/usr/bin/env python3

"""
Summarizes recent PRs based on their GitHub labels.

The result can be copy-pasted into CHANGELOG.md, though it often needs some manual editing too.
"""

import re
import sys
from dataclasses import dataclass
from typing import Any, List, Optional, Tuple

import requests
from git import Repo  # pip install GitPython
from tqdm import tqdm

OWNER = "rerun-io"
REPO = "rerun"
COMMIT_RANGE = "latest..HEAD"
INCLUDE_LABELS = False  # It adds quite a bit of visual noise
OFFICIAL_RERUN_DEVS = [
    "emilk",
    "jleibs",
    "nikolausWest",
    "teh-cmc",
    "Wumpf",
]


@dataclass
class PrInfo:
    gh_user_name: str
    pr_title: str
    labels: List[str]


def get_github_token() -> str:
    import os

    token = os.environ.get("GH_ACCESS_TOKEN", "")
    if token != "":
        return token

    home_dir = os.path.expanduser("~")
    token_file = os.path.join(home_dir, ".githubtoken")

    try:
        with open(token_file, "r") as f:
            token = f.read().strip()
        return token
    except Exception:
        pass

    print("ERROR: expected a GitHub token in the environment variable GH_ACCESS_TOKEN or in ~/.githubtoken")
    sys.exit(1)


# Slow
def fetch_pr_info(pr_number: int) -> Optional[PrInfo]:
    url = f"https://api.github.com/repos/{OWNER}/{REPO}/pulls/{pr_number}"
    gh_access_token = get_github_token()
    headers = {"Authorization": f"Token {gh_access_token}"}
    response = requests.get(url, headers=headers)
    json = response.json()

    # Check if the request was successful (status code 200)
    if response.status_code == 200:
        labels = [label["name"] for label in json["labels"]]
        gh_user_name = json["user"]["login"]
        return PrInfo(gh_user_name=gh_user_name, pr_title=json["title"], labels=labels)
    else:
        print(f"ERROR: {response.status_code} - {json['message']}")
        return None


def commit_title_pr_number(commit: Any) -> Tuple[str, Optional[int]]:
    match = re.match(r"(.*) \(#(\d+)\)", commit.summary)
    if match:
        return (str(match.group(1)), int(match.group(2)))
    else:
        return (commit.summary, None)


def print_section(title: str, items: List[str]) -> None:
    if 0 < len(items):
        print(f"#### {title}")
        for line in items:
            print(f"- {line}")
    print()


def main() -> None:
    repo = Repo(".")
    commits = list(repo.iter_commits(COMMIT_RANGE))
    commits.reverse()  # Most recent last

    # Sections:
    analytics = []
    enhancement = []
    bugs = []
    dev_experience = []
    docs = []
    examples = []
    misc = []
    performance = []
    python = []
    renderer = []
    rfc = []
    rust = []
    ui = []
    viewer = []
    web = []

    for commit in tqdm(commits, desc="Processing commits"):
        (title, pr_number) = commit_title_pr_number(commit)
        if pr_number is None:
            # Someone committed straight to main:
            summary = f"{title} [{commit.hexsha}](https://github.com/{OWNER}/{REPO}/commit/{commit.hexsha})"
            misc.append(summary)
        else:
            pr_info = fetch_pr_info(pr_number)
            title = pr_info.pr_title if pr_info else title  # We prefer the PR title if available
            labels = pr_info.labels if pr_info else []

            summary = f"{title} [#{pr_number}](https://github.com/{OWNER}/{REPO}/pull/{pr_number})"

            if INCLUDE_LABELS and 0 < len(labels):
                summary += f" ({', '.join(labels)})"

            if pr_info is not None:
                gh_user_name = pr_info.gh_user_name
                if gh_user_name not in OFFICIAL_RERUN_DEVS:
                    summary += f" (thanks [@{gh_user_name}](https://github.com/{gh_user_name})!)"

            if labels == ["⛴ release"]:
                continue  # Ignore release PRs

            added = False

            # Some PRs can show up underm multiple sections:
            if "🐍 python API" in labels:
                python.append(summary)
                added = True
            if "🦀 rust SDK" in labels:
                rust.append(summary)
                added = True
            if "📊 analytics" in labels:
                analytics.append(summary)
                added = True

            if not added:
                # Put the remaining PRs under just one section:
                if "🪳 bug" in labels or "💣 crash" in labels:
                    bugs.append(summary)
                elif "📉 performance" in labels:
                    performance.append(summary)
                elif "examples" in labels:
                    examples.append(summary)
                elif "📖 documentation" in labels:
                    docs.append(summary)
                elif "ui" in labels:
                    ui.append(summary)
                elif "📺 re_viewer" in labels:
                    viewer.append(summary)
                elif "🔺 re_renderer" in labels:
                    renderer.append(summary)
                elif "🕸️ web" in labels:
                    web.append(summary)
                elif "enhancement" in labels:
                    enhancement.append(summary)
                elif "🧑‍💻 dev experience" in labels:
                    dev_experience.append(summary)
                elif "💬 discussion" in labels:
                    rfc.append(summary)
                elif not added:
                    misc.append(summary)

    print()
    # Most interesting first:
    print_section("🐍 Python SDK", python)
    print_section("🦀 Rust SDK", rust)
    print_section("🪳 Bug Fixes", bugs)
    print_section("🚀 Performance Improvements", performance)
    print_section("🧑‍🏫 Examples", examples)
    print_section("📚 Docs", docs)
    print_section("🖼 UI Improvements", ui)
    print_section("🤷‍♂️ Other Viewer Improvements", viewer)
    print_section("🕸️ Web", web)
    print_section("🎨 Renderer Improvements", renderer)
    print_section("✨ Other Enhancement", enhancement)
    print_section("📈 Analytics", analytics)
    print_section("🗣 Merged RFCs", rfc)
    print_section("🧑‍💻 Dev-experience", dev_experience)
    print_section("🤷‍♂️ Other", misc)


if __name__ == "__main__":
    main()
