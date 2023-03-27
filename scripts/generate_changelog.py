#!/usr/bin/env python3

"""
Summarizes recent PRs based on their GitHub labels.

The result can be copy-pasted into CHANGELOG.md, though it often needs some manual editing too.
"""

import re
import sys
from typing import Any, List, Optional, Tuple

import requests
from git import Repo  # pip install GitPython
from tqdm import tqdm


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

    print("ERROR: expected a github token in the environment variable GH_ACCESS_TOKEN or in ~/.githubtoken")
    sys.exit(1)


OWNER = "rerun-io"
REPO = "rerun"
COMMIT_RANGE = "latest..HEAD"


def pr_title_labels(pr_number: int) -> Tuple[Optional[str], List[str]]:
    url = f"https://api.github.com/repos/{OWNER}/{REPO}/pulls/{pr_number}"
    gh_access_token = get_github_token()
    headers = {"Authorization": f"Token {gh_access_token}"}
    response = requests.get(url, headers=headers)
    json = response.json()

    # Check if the request was successful (status code 200)
    if response.status_code == 200:
        labels = [label["name"] for label in json["labels"]]
        return (json["title"], labels)
    else:
        print(f"ERROR: {response.status_code} - {json['message']}")
        return (None, [])


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


repo = Repo(".")
commits = list(repo.iter_commits(COMMIT_RANGE))
commits.reverse()

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
        (pr_title, labels) = pr_title_labels(pr_number)
        title = pr_title or title  # We prefer the PR title if available
        summary = f"{title} [#{pr_number}](https://github.com/{OWNER}/{REPO}/pull/{pr_number})"

        added = False

        if labels == ["⛴ release"]:
            # Ignore release PRs
            continue

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
print_section("📈 Analytics", analytics)
print_section("🪳 Bug Fixes", bugs)
print_section("🚀 Performance Improvements", performance)
print_section("🧑‍🏫 Examples", examples)
print_section("📚 Docs", docs)
print_section("🖼 UI Improvements", ui)
print_section("🤷‍♂️ Other Viewer Improvements", viewer)
print_section("🕸️ web", web)
print_section("🎨 Renderer Improvements", renderer)
print_section("✨ Other Enhancement", enhancement)
print_section("🗣 Merged RFCs", rfc)
print_section("🧑‍💻 Dev-experience", dev_experience)
print_section("🤷‍♂️ Other", misc)
