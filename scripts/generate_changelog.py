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
    "gavrelina",
    "grtlr",
    "jleibs",
    "jprochazk",
    "nikolausWest",
    "teh-cmc",
    "Wumpf",
    "zehiko",
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
        with open(token_file, encoding="utf8") as f:
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


def commit_range(new_version: str) -> str:
    parts = new_version.split(".")
    assert len(parts) == 3, "Expected version to be on the format X.Y.Z"
    major = int(parts[0])
    minor = int(parts[1])
    patch = int(parts[2])

    if 0 < patch:
        # A patch release.
        # Include changes since last patch release.
        # This assumes we've cherry-picked stuff for this release.
        diff_since_version = f"0.{minor}.{patch - 1}"
    elif 0 < minor:
        # A minor release
        # The diff should span everything since the last minor release.
        # The script later excludes duplicated automatically, so we don't include stuff that
        # was part of intervening patch releases.
        diff_since_version = f"{major}.{minor - 1}.0"
    else:
        # A major release
        # The diff should span everything since the last major release.
        # The script later excludes duplicated automatically, so we don't include stuff that
        # was part of intervening minor/patch releases.
        diff_since_version = f"{major - 1}.{minor}.0"

    return f"{diff_since_version}..HEAD"


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a changelog.")
    parser.add_argument("--version", required=True, help="The version of the new release, e.g. 0.42.0")
    args = parser.parse_args()

    # Because how we branch, we sometimes get duplicate commits in the changelog unless we check for it
    previous_changelog = open("CHANGELOG.md", encoding="utf8").read()

    repo = Repo(".")
    commits = list(repo.iter_commits(commit_range(args.version)))
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
    log_api = []
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

            if "include in changelog" not in labels:
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

            # Some PRs can show up under multiple sections:
            if "🪵 Log & send APIs" in labels:
                log_api.append(summary)
                added = True
            else:
                if "🌊 C++ API" in labels:
                    cpp.append(summary)
                    added = True
                if "🐍 Python API" in labels:
                    python.append(summary)
                    added = True
                if "🦀 Rust API" in labels:
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

    # NOTE: we inentionally add TODO:s with names below, which the CI will not be happy about. Hence the # NOLINT suffixes
    print("TODO: add link to release video")  # NOLINT
    print()
    print("📖 Release blogpost: TODO: add link")  # NOLINT
    print()
    print("🧳 Migration guide: TODO: add link")  # NOLINT
    print()
    print("### ✨ Overview & highlights")
    print("TODO: fill in")  # NOLINT
    print()
    print("### ⚠️ Breaking changes")
    print("TODO: fill in")  # NOLINT
    print("🧳 Migration guide: TODO: add link (yes, again)")  # NOLINT
    print()
    print("### 🔎 Details")
    print()

    # Most interesting first:
    print_section("🪵 Log API", log_api)
    print_section("🌊 C++ API", cpp)
    print_section("🐍 Python API", python)
    print_section("🦀 Rust API", rust)
    print_section("🪳 Bug fixes", bugs)
    print_section("🌁 Viewer improvements", viewer)
    print_section("🚀 Performance improvements", performance)
    print_section("🧑‍🏫 Examples", examples)
    print_section("📚 Docs", docs)
    print_section("🖼 UI improvements", ui)
    print_section("🕸️ Web", web)
    print_section("🎨 Renderer improvements", renderer)
    print_section("✨ Other enhancement", enhancement)
    print_section("📈 Analytics", analytics)
    print_section("🗣 Merged RFCs", rfc)
    print_section("🧑‍💻 Dev-experience", dev_experience)
    print_section("🗣 Refactors", refactor)
    print_section("📦 Dependencies", dependencies)
    print_section("🤷‍ Other", misc)

    print()
    print_section("Chronological changes (don't include these)", chronological)


if __name__ == "__main__":
    main()
