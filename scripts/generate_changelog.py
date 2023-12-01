#!/usr/bin/env python3

"""
Summarizes PRs since `latest` branch, grouping them based on their GitHub labels.

If the result is not satisfactory, you can edit the original PR titles and labels.
You can add the `exclude from changelog` label to minor PRs that are not of interest to our users.

Finally, copy-paste the output into `CHANGELOG.md` and add a high-level summary to the top.
"""
from __future__ import annotations

import argparse
import multiprocessing
import re
import sys
from dataclasses import dataclass
from typing import Any

import requests
from git import Repo  # pip install GitPython
from tqdm import tqdm

OWNER = "rerun-io"
REPO = "rerun"
INCLUDE_LABELS = False  # It adds quite a bit of visual noise
OFFICIAL_RERUN_DEVS = [
    "abey79",
    "emilk",
    "jleibs",
    "jprochazk",
    "nikolausWest",
    "teh-cmc",
    "Wumpf",
]


def eprint(*args, **kwargs) -> None:  # type: ignore
    print(*args, file=sys.stderr, **kwargs)  # type: ignore


@dataclass
class PrInfo:
    gh_user_name: str
    pr_title: str
    labels: list[str]


@dataclass
class CommitInfo:
    hexsha: str
    title: str
    pr_number: int | None


def get_github_token() -> str:
    import os

    token = os.environ.get("GH_ACCESS_TOKEN", "")
    if token != "":
        return token

    home_dir = os.path.expanduser("~")
    token_file = os.path.join(home_dir, ".githubtoken")

    try:
        with open(token_file) as f:
            token = f.read().strip()
        return token
    except Exception:
        pass

    eprint("ERROR: expected a GitHub token in the environment variable GH_ACCESS_TOKEN or in ~/.githubtoken")
    sys.exit(1)


# Slow
def fetch_pr_info_from_commit_info(commit_info: CommitInfo) -> PrInfo | None:
    if commit_info.pr_number is None:
        return None
    else:
        return fetch_pr_info(commit_info.pr_number)


# Slow
def fetch_pr_info(pr_number: int) -> PrInfo | None:
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
        eprint(f"ERROR {url}: {response.status_code} - {json['message']}")
        return None


def get_commit_info(commit: Any) -> CommitInfo:
    match = re.match(r"(.*) \(#(\d+)\)", commit.summary)
    if match:
        return CommitInfo(
            hexsha=commit.hexsha,
            title=str(match.group(1)),
            pr_number=int(match.group(2)),
        )
    else:
        return CommitInfo(hexsha=commit.hexsha, title=commit.summary, pr_number=None)


def print_section(title: str, items: list[str]) -> None:
    if 0 < len(items):
        print(f"#### {title}")
        for line in items:
            print(f"- {line}")
        print()


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a changelog.")
    parser.add_argument("--commit-range", default="latest..HEAD", help="e.g. 0.8.0..HEAD")
    args = parser.parse_args()

    # Because how we branch, we sometimes get duplicate commits in the changelog unless we check for it
    previous_changelog = open("CHANGELOG.md").read()

    repo = Repo(".")
    commits = list(repo.iter_commits(args.commit_range))
    commits.reverse()  # Most recent last
    commit_infos = list(map(get_commit_info, commits))

    pool = multiprocessing.Pool()
    pr_infos = list(
        tqdm(
            pool.imap(fetch_pr_info_from_commit_info, commit_infos),
            total=len(commit_infos),
            desc="Fetch PR info commits",
        )
    )

    chronological = []

    # Sections:
    analytics = []
    bugs = []
    cpp = []
    dependencies = []
    dev_experience = []
    docs = []
    enhancement = []
    examples = []
    misc = []
    performance = []
    python = []
    refactor = []
    renderer = []
    rfc = []
    rust = []
    ui = []
    viewer = []
    web = []

    for commit_info, pr_info in zip(commit_infos, pr_infos):
        hexsha = commit_info.hexsha
        title = commit_info.title
        pr_number = commit_info.pr_number

        if pr_number is None:
            # Someone committed straight to main:
            summary = f"{title} [{hexsha}](https://github.com/{OWNER}/{REPO}/commit/{hexsha})"
            if f"[{hexsha}]" in previous_changelog:
                print(f"Ignoring dup: {summary}")
                continue

            chronological.append(summary)
            misc.append(summary)
        else:
            title = pr_info.pr_title if pr_info else title  # We prefer the PR title if available
            title = title.rstrip(".").strip()  # Some PR end with an unnecessary period

            labels = pr_info.labels if pr_info else []

            if "exclude from changelog" in labels:
                continue

            summary = f"{title} [#{pr_number}](https://github.com/{OWNER}/{REPO}/pull/{pr_number})"

            if f"[#{pr_number}]" in previous_changelog:
                eprint(f"Ignoring dup: {summary}")
                continue

            chronological.append(f"{summary} {hexsha}")

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
            if "C/C++ SDK" in labels:
                cpp.append(summary)
                added = True
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
                if "examples" in labels:
                    examples.append(summary)
                elif "🪳 bug" in labels or "💣 crash" in labels or "🦟 regression" in labels:
                    bugs.append(summary)
                elif "📉 performance" in labels:
                    performance.append(summary)
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
                elif "🚜 refactor" in labels:
                    refactor.append(summary)
                elif "🧑‍💻 dev experience" in labels:
                    dev_experience.append(summary)
                elif "💬 discussion" in labels:
                    rfc.append(summary)
                elif "dependencies" in labels:
                    dependencies.append(summary)
                elif not added:
                    misc.append(summary)

    print()

    # Most interesting first:
    print_section("🌊 C++ SDK", cpp)
    print_section("🐍 Python SDK", python)
    print_section("🦀 Rust SDK", rust)
    print_section("🪳 Bug Fixes", bugs)
    print_section("🌁 Viewer Improvements", viewer)
    print_section("🚀 Performance Improvements", performance)
    print_section("🧑‍🏫 Examples", examples)
    print_section("📚 Docs", docs)
    print_section("🖼 UI Improvements", ui)
    print_section("🕸️ Web", web)
    print_section("🎨 Renderer Improvements", renderer)
    print_section("✨ Other Enhancement", enhancement)
    print_section("📈 Analytics", analytics)
    print_section("🗣 Merged RFCs", rfc)
    print_section("🧑‍💻 Dev-experience", dev_experience)
    print_section("🗣 Refactors", refactor)
    print_section("📦 Dependencies", dependencies)
    print_section("🤷‍♂️ Other", misc)

    print()
    print_section("Chronological changes (don't include these)", chronological)


if __name__ == "__main__":
    main()
